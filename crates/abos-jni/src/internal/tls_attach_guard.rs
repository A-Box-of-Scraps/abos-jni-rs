//! Thread attachment tracking with automatic detachment through non-Windows TLS destructors.

use std::{
	cell::RefCell,
	thread::{Thread, current},
};

use log::error;

use crate::{
	JavaVM,
	errors::{Error, Result},
	vm::{AttachConfig, AttachGuard, sys_attach_current_thread, sys_detach_current_thread},
};

thread_local! {
	// There's a false-positive Clippy bug: https://github.com/rust-lang/rust-clippy/issues/13422
	#[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local))]
	static TLS_ATTACH_GUARD: RefCell<Option<TLSAttachGuard>> = const { RefCell::new(None) }
}

/// Data stored in TLS for automatic thread detachment.
///
/// Environment pointers are not retained: external code may detach the thread when no
/// `AttachGuard`s are active, invalidating the pointer before the TLS destructor runs.
#[derive(Debug)]
struct TLSAttachGuard {
	/// Captured at attachment because `thread::current()` can panic during TLS destruction.
	thread: Thread,
}

impl TLSAttachGuard {
	/// Detach the current thread after checking there are no active [`AttachGuard`]s
	///
	/// # Safety
	/// The caller must prevent `Drop` from running after an explicit detach.
	unsafe fn detach_impl(&self) -> Result<()> {
		// SAFETY: The caller prevents a second detach, and this thread-local guard belongs to the current thread.
		unsafe { sys_detach_current_thread(None, &self.thread) }
	}
}

impl Drop for TLSAttachGuard {
	fn drop(&mut self) {
		// SAFETY: Drop runs once on the thread that owns this TLS guard.
		if let Err(e) = unsafe { self.detach_impl() } {
			error!(
				"Error detaching current thread: {:#?}\nThread {} id={:?}",
				e,
				self.thread.name().unwrap_or_default(),
				self.thread.id(),
			);
		}
	}
}

/// Attach the current thread to the JVM and register TLS cleanup for thread termination.
///
/// The caller must first use `GetEnv` to confirm that the thread is detached.
///
/// # Panics
///
/// Panics if any `AttachGuard`s are active.
pub unsafe fn tls_attach_current_thread<'local>(
	java_vm: &JavaVM,
	config: &AttachConfig,
) -> Result<AttachGuard<'local>> {
	let thread: Thread = current();

	let env: *mut *const abos_jni_sys::JNINativeInterface_ =
		TLS_ATTACH_GUARD.with(move |f| -> abos_jni::errors::Result<*mut abos_jni::sys::JNIEnv> {
			// External code may have detached the thread while leaving its TLS guard.
			// Reuse the counter entry when reattaching.
			let inc_attached_count: bool = f.borrow_mut().take().is_none_or(|guard| {
				// Do not let the old guard detach the thread again.
				std::mem::forget(guard);
				false
			});
			// SAFETY: The caller upholds the attachment contract; the config and thread remain live during attachment.
			let env: *mut *const abos_jni_sys::JNINativeInterface_ =
				unsafe { sys_attach_current_thread(java_vm, config, &thread, inc_attached_count)? };
			*f.borrow_mut() = Some(TLSAttachGuard { thread: current() });
			Ok(env)
		})?;

	// SAFETY: The JNI environment was obtained for this thread and the returned guard tracks its scope.
	Ok(unsafe { AttachGuard::from_unowned(env) })
}

/// Detach the current thread if it has a TLS attachment guard.
///
/// Returns `Error::ThreadAttachmentGuarded` if any `AttachGuard`s are active.
pub fn tls_detach_current_thread() -> Result<()> {
	if JavaVM::thread_attach_guard_level() != 0 {
		return Err(Error::ThreadAttachmentGuarded);
	}

	TLS_ATTACH_GUARD.with(move |f| {
		f.borrow_mut().take().map_or_else(
			|| Ok(()),
			|guard| {
				// SAFETY: Forgetting the guard below prevents a second detach through `Drop`.
				let res: std::result::Result<(), crate::errors::Error> = unsafe { guard.detach_impl() };
				std::mem::forget(guard);
				res
			},
		)
	})
}
