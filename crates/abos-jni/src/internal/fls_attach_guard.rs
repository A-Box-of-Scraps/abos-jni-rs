//! Windows thread attachment tracking through Fiber Local Storage (FLS).
//!
//! FLS cleanup avoids the loader-lock deadlock possible with TLS destructors: detachment
//! can wait for a JVM safepoint while a Java thread needs the loader lock to reach it.
//! See <https://github.com/jni-rs/jni-rs/issues/701>.
//!
//! Fiber switching, fiber deletion, and external thread detachment must not occur while
//! any `AttachGuard` is active. They can invalidate the environment pointer, local frames,
//! and pending exception state in use by Rust code.
//!
//! External detachment is supported when no guards are active. `attach_current_thread()`
//! checks the JNI attachment state before reusing an unguarded attachment.

use std::{
	ptr,
	sync::OnceLock,
	thread::{Thread, current},
};

use log::debug;

use crate::{
	AttachGuard, JavaVM,
	errors::{Error, JniError, Result},
	vm::{sys_attach_current_thread, sys_detach_current_thread},
};

use crate::windows_sys::{FlsAlloc, FlsGetValue, FlsSetValue};

/// Shared FLS index, initialized on first attachment.
static FLS_INDEX: OnceLock<u32> = OnceLock::new();

/// Data stored in FLS for each attached fiber.
///
/// Environment pointers are not retained: another fiber or external code can detach
/// the thread when no `AttachGuard`s are active, invalidating the shared pointer.
#[derive(Debug)]
struct FlsAttachData {
	thread: Thread,
}

/// Detach the current thread when Windows destroys a fiber or frees its FLS slot.
///
/// Multiple fibers can hold attachment data for the same thread after external detach
/// and reattach operations. Each callback releases one logical attachment; detaching
/// an already detached thread is a no-op at the JNI level.
///
/// # Panics
///
/// Panics if any `AttachGuard`s are active. Unwinding across this callback's ABI aborts.
unsafe extern "system" fn fls_callback(data: *const core::ffi::c_void) {
	if data.is_null() {
		return;
	}

	// Safety: We only ever store Box<FlsAttachData> in FLS
	let attach_data: Box<FlsAttachData> = unsafe { Box::from_raw(data as *mut FlsAttachData) };

	// SAFETY INVARIANT: Fiber switching or freeing must not occur while an AttachGuard is in scope.
	let guard_level: usize = crate::JavaVM::thread_attach_guard_level();
	assert!(
		guard_level == 0,
		indoc::indoc! {
			"FATAL: FLS callback invoked with active AttachGuard (nest_level={}). \
			Fiber switching must not occur while an AttachGuard is in scope. \
			Thread: {} ({:?})"
		},
		guard_level,
		attach_data.thread.name().unwrap_or_default(),
		attach_data.thread.id()
	);

	// No environment cross-check: another fiber may already have detached the thread.
	// Logging here could access already-destroyed TLS, even when reporting an error.
	// SAFETY: No attachment guards remain, as checked above; no environment is used after detaching.
	drop(unsafe { sys_detach_current_thread(None) });
}

/// Return the shared FLS index, allocating it if necessary.
fn get_or_init_fls_index() -> Result<u32> {
	if let Some(&index) = FLS_INDEX.get() {
		return Ok(index);
	}

	// Safety: fls_callback is a valid callback function
	let new_index: u32 = unsafe { FlsAlloc(Some(fls_callback)) };

	if new_index == u32::MAX {
		return Err(Error::JniCall(JniError::Unknown));
	}

	Ok(*FLS_INDEX.get_or_init(|| new_index))
}

/// Attach the current thread and register cleanup in the current fiber's FLS slot.
///
/// The caller must first use `GetEnv` to confirm that the thread is detached.
///
/// After external detachment, multiple fibers can independently reattach the same thread.
/// `ATTACHED_THREADS` counts their FLS records rather than OS threads. Each cleanup releases
/// one record, even if another fiber has already detached the thread.
///
/// # Panics
///
/// Panics if any `AttachGuard`s are active.
pub unsafe fn fls_attach_current_thread<'local>(
	java_vm: &crate::JavaVM,
	config: &crate::vm::AttachConfig,
) -> Result<crate::vm::AttachGuard<'local>> {
	// Fiber attachment while a guard is active can invalidate its JNI state.
	let guard_level: usize = JavaVM::thread_attach_guard_level();
	assert!(
		guard_level == 0,
		indoc::indoc! {
			"FATAL: fls_attach_current_thread called with active AttachGuard (nest_level={}). \
			Fiber attachment/switching must not occur while an AttachGuard is in scope. \
			Thread: {} ({:?})"
		},
		guard_level,
		current().name().unwrap_or_default(),
		current().id()
	);

	let fls_index: u32 = get_or_init_fls_index()?;

	// Safety: fls_index is valid
	let existing: *mut std::ffi::c_void = unsafe { FlsGetValue(fls_index) };
	// External code may have detached the thread while leaving its FLS data.
	// Reuse the counter entry when reattaching.
	let inc_attached_count: bool = existing.is_null();

	let thread: Thread = current();
	// SAFETY: No attachment guards are active, and the caller upholds the returned guard's scope requirements.
	let env: *mut *const abos_jni_sys::JNINativeInterface_ =
		unsafe { sys_attach_current_thread(java_vm, config, &thread, inc_attached_count)? };

	// Do not retain env pointers: fibers sharing an attachment can leave them stale.
	let attach_data: Box<FlsAttachData> = Box::new(FlsAttachData { thread: current() });

	let data_ptr: *mut std::ffi::c_void = Box::into_raw(attach_data).cast::<core::ffi::c_void>();

	// Safety: fls_index is valid, data_ptr is a valid pointer
	let result: i32 = unsafe { FlsSetValue(fls_index, data_ptr) };

	if result == 0 {
		// SAFETY: FLS did not take ownership; reclaim the allocation produced by Box::into_raw above.
		unsafe {
			drop(Box::from_raw(data_ptr.cast::<FlsAttachData>()));
		}
		return Err(Error::JniCall(JniError::Unknown));
	}

	debug!(
		"Attached fiber via FLS: {} ({:?}). {} threads attached",
		current().name().unwrap_or_default(),
		current().id(),
		java_vm.threads_attached(),
	);

	// SAFETY: The environment belongs to this thread; FLS owns the attachment and the caller guarantees the guard scope.
	Ok(unsafe { AttachGuard::from_unowned(env) })
}

/// Detach the current thread if the current fiber has an FLS attachment record.
///
/// Clears the FLS slot before detaching so cleanup cannot detach it again.
/// Returns `Error::ThreadAttachmentGuarded` if any `AttachGuard`s are active.
pub fn fls_detach_current_thread() -> Result<()> {
	if JavaVM::thread_attach_guard_level() != 0 {
		return Err(Error::ThreadAttachmentGuarded);
	}

	let Some(&fls_index) = FLS_INDEX.get() else {
		return Ok(()); // FLS never initialized, so not attached
	};

	// Safety: fls_index is valid
	let data_ptr: *mut std::ffi::c_void = unsafe { FlsGetValue(fls_index) };
	if data_ptr.is_null() {
		return Ok(());
	}

	// Clear the FLS slot first (before detaching) to prevent the callback from running
	// Safety: fls_index is valid
	let result: i32 = unsafe { FlsSetValue(fls_index, ptr::null_mut()) };
	// Failure leaves the callback owning this record; continuing would risk duplicate cleanup.
	assert_ne!(result, 0, "FATAL: Failed to clear FLS slot during detach");

	// SAFETY: `data_ptr` is the boxed record retrieved from the cleared FLS slot.
	unsafe { drop(Box::from_raw(data_ptr.cast::<FlsAttachData>())) };

	// No environment cross-check: FLS records outlive external detachment.
	// SAFETY: No attachment guards remain, and the FLS slot was cleared before detaching.
	unsafe { sys_detach_current_thread(None)? };
	Ok(())
}
