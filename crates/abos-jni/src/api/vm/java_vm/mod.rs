use std::{
	cell::Cell,
	ptr,
	sync::atomic::{AtomicUsize, Ordering},
	thread::{Thread, current},
};

use log::debug;

use crate::{
	Env, JNIVersion,
	errors::{Error, JniError, Result, jni_error_code_to_result},
	objects::{Global, JObject, JThrowable, Reference},
	strings::{JNIStr, JNIString},
	sys,
};

#[cfg(doc)]
use {
	crate::api::env::{self, EnvUnowned},
	crate::objects,
};

/// The capacity of local frames, allocated for attached threads by default. Same as the default
/// value Hotspot uses when calling native Java methods.
pub const DEFAULT_LOCAL_FRAME_CAPACITY: usize = 32;

/// The `jni-rs` crate makes the assumption that it's not possible to create more than one Java VM
/// per-process, or even re-initialize a `JavaVM` that is "destroyed".
///
/// This allows us to save a global pointer for the `JavaVM`.
///
/// We also guarantee that if you currently have an [`AttachGuard`] thread attachment (or a `Env`
/// reference), that implies that [`JavaVM::singleton()`] has been initialized and will return a
/// valid [`JavaVM`].
///
/// For example, this guarantee is relied on internally to avoid redundantly saving `JavaVM` pointers
/// if know we can assume that `JavaVM::singleton()` will return a `JavaVM` when needed.
static JAVA_VM_SINGLETON: std::sync::OnceLock<JavaVM> = std::sync::OnceLock::new();

/// The Java VM API, including (optional) [Invocation API][invocation-api] support.
///
/// An existing `JavaVM` can be obtained either via [`JavaVM::singleton`], or [`Env::get_java_vm`]
/// in an already attached thread, or a new VM can be [launched](#launching-jvm-from-rust) via
/// [`JavaVM::new`].
///
/// ## Minimum supported JNI version is 1.4
///
/// The implementation of this crate assumes your Java VM supports at least JNI >= 1.4
///
/// The implementation wouldn't be able to call `ExceptionCheck` without JNI 1.2 and requiring >=
/// 1.4 means we don't need any runtime version checks for the direct byte buffer APIs.
///
/// Since `GetVersion` requires a [`Env`] and is not one of the JNI APIs that is safe to use with
/// pending exceptions then the implementation is not always able to explicitly assert the supported
/// version.
///
/// ## Attaching Native Threads
///
/// Your application always needs to explicitly attach `jni-rs` to the current thread before it can
/// access the [`Env`] API (most of the interesting APIs are under [`Env`]).
///
/// If you're implementing a native/foreign method then JNI will pass a thread attachment (in the
/// form of a raw [`sys::JNIEnv`] pointer that should be captured using the
/// [`env::EnvUnowned`] type) and converted into a [`Env`] reference via
/// [`env::EnvUnowned::with_env`])
///
/// Note: [`Env`] is not a `#[transparent]` wrapper over a [`sys::JNIEnv`] pointer.
///
/// The attachment of the current thread is always represented via an [`AttachGuard`] which blocks
/// the thread from being detached and acts as a marker for a JNI stack frame.
///
/// [`Env`] is only ever exposed in the public API by-reference, and will always borrow from an
/// [`AttachGuard`] such that it's lifetime is tied to a fixed JNI stack frame.
///
/// Unless you are using `unsafe` APIs though, the [`AttachGuard`] itself will usually be hidden,
/// and you will get a [`Env`] reference (that borrows from a hidden [`AttachGuard`]).
///
/// This library supports these modes of attachment:
/// * A permanent attachment with [`JavaVM::attach_current_thread`]. The thread will automatically
///   detach itself before it terminates (recommended for attaching in native threads).
/// * A scoped attachment with [`JavaVM::attach_current_thread_for_scope`]. The thread will
///   automatically detach itself once your given closure returns (and the hidden [`AttachGuard`]
///   is dropped).
/// * Implicit ("unowned") attachments, for use in native methods. The thread will never get
///   explicitly detached by `jni-rs` if we have implicit attachment.
///
/// ### Local Reference Management
///
/// Remember that the native thread attached to the VM must manage local references
/// carefully, i.e., do not allocate an excessive number of references and release them promptly when
/// they are no longer needed to enable the GC to collect them.
///
/// A common approach is to push appropriately-sized local frames for larger
/// code fragments (see [`Env::with_local_frame`] or [`JavaVM::with_local_frame`])
/// and [`objects::Auto`] for temporary references in loops.
///
/// See also the [JNI specification][spec-references] for details on referencing Java objects.
///
/// ## Launching JVM from Rust
///
/// To [launch][launch-vm] a JVM from a native process, enable the `invocation` feature in the
/// Cargo.toml:
///
/// ```toml
/// jni = { version = "0.22.1", features = ["invocation"] }
/// ```
///
/// The application will be able to use [`JavaVM::new`] which will dynamically load a `jvm` library
/// (which is distributed with the JVM) at runtime:
///
/// ```rust,ignore-aarch64-linux-android
/// # use abos_jni::errors;
/// # //
/// # // Ignore this test without invocation feature, so that simple `cargo test` works
/// # #[cfg(feature = "invocation")]
/// # fn main() -> errors::StartJvmResult<()> {
/// # use abos_jni::{jni_sig, jni_str, AttachGuard, objects::JValue, InitArgsBuilder, Env, JNIVersion, JavaVM, sys::jint};
/// # //
/// // Build the VM properties
/// let jvm_args = InitArgsBuilder::new()
///           // Pass the JNI API version (default is 8)
///           .version(JNIVersion::V1_8)
///           // You can additionally pass any JVM options (standard, like a system property,
///           // or VM-specific).
///           // Here we enable some extra JNI checks useful during development
///           .option("-Xcheck:jni")
///           .build()
///           .unwrap();
///
/// // Create a new VM
/// let jvm = JavaVM::new(jvm_args)?;
///
/// // Attach the current thread to call to the JavaVM
/// jvm.attach_current_thread(|env| -> errors::Result<()> {
///     // Call Java Math#abs(-10)
///     let x = JValue::from(-10);
///     let val: jint = env.call_static_method(jni_str!("java/lang/Math"), jni_str!("abs"), jni_sig!("(I)I"), &[x])?
///         .i()?;
///
///     assert_eq!(val, 10);
///     Ok(())
/// });
///
/// # Ok(()) }
/// #
/// # // This is a stub that gets run instead if the invocation feature is not built
/// # #[cfg(not(feature = "invocation"))]
/// # fn main() {}
/// ```
///
/// At runtime, the JVM installation path is determined via the [java-locator] crate:
/// 1. By the `JAVA_HOME` environment variable, if it is set.
/// 2. Otherwise — from `java` output.
///
/// It is recommended to set `JAVA_HOME`
///
/// For the operating system to correctly load the `jvm` library it may also be necessary to update
/// the path that the OS uses to find dependencies of the `jvm` library.
/// * On **Windows**, append the path to `$JAVA_HOME/bin` to the `PATH` environment variable.
/// * On **`MacOS`**, append the path to `libjvm.dylib` to `LD_LIBRARY_PATH` environment variable.
/// * On **Linux**, append the path to `libjvm.so` to `LD_LIBRARY_PATH` environment variable.
///
/// The exact relative path to `jvm` library is version-specific.
///
/// [invocation-api]: https://docs.oracle.com/en/java/javase/12/docs/specs/jni/invocation.html
/// [get-vm]: struct.Env.html#method.get_java_vm
/// [launch-vm]: struct.JavaVM.html#method.new
/// [act]: struct.JavaVM.html#method.attach_current_thread
/// [actp]: struct.JavaVM.html#method.attach_current_thread_permanently
/// [spec-references]:
///     https://docs.oracle.com/en/java/javase/12/docs/specs/jni/design.html#referencing-java-objects
/// [java-locator]: https://crates.io/crates/java-locator
#[derive(Debug, Clone)]
pub struct JavaVM(*mut sys::JavaVM);

// SAFETY: A JavaVM handle is VM-wide; each thread obtains its own JNI environment before using it.
unsafe impl Send for JavaVM {}
// SAFETY: The invocation interface supports concurrent threads, and Env values remain thread-local.
unsafe impl Sync for JavaVM {}

/// Tracks the number of logical attachments managed by this crate.
///
/// This is unconditionally incremented for each call to `sys_attach_current_thread` and decremented
/// for each call to `sys_detach_current_thread`.
///
/// **Windows with FLS caveats:**
///
/// On Windows with FLS, this still typically corresponds to the number of OS threads attached,
/// because only one fiber per-OS-thread will be responsible for attaching to the JVM (others would
/// observe that something has already attached the OS thread).
///
/// In the unlikely situation where JNI is manually used to detach an OS thread (allowed if there
/// are no active `AttachGuards`), then multiple fibers could separately attach the same OS thread and
/// both increment this counter. They will also both decrement the counter when they are freed (or
/// if `detach_current_thread()` is called from each fiber).
pub(super) static ATTACHED_THREADS: AtomicUsize = AtomicUsize::new(0);

pub unsafe fn sys_attach_current_thread(
	vm: &JavaVM,
	config: &AttachConfig,
	thread: &Thread,
	inc_attached_count: bool,
) -> Result<*mut sys::JNIEnv> {
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);
	let mut env_ptr = ptr::null_mut();

	let mut args = sys::JavaVMAttachArgs {
		version: JNIVersion::V1_4.into(),
		name: config.thread_name.map_or_else(
			|| {
				config
					.deprecated_thread_name
					.as_ref()
					.map_or(ptr::null_mut(), |s| s.as_ptr().cast_mut())
			},
			|s| s.as_ptr().cast_mut(),
		),
		group: config.group.as_ref().map_or(ptr::null_mut(), Reference::as_raw),
	};
	// SAFETY: The live VM receives writable out-parameter storage and attachment arguments valid for the call.
	let res = unsafe {
		java_vm_call_unchecked!(
			vm,
			v1_1,
			AttachCurrentThread,
			&raw mut env_ptr,
			(&raw mut args).cast::<core::ffi::c_void>()
		)
	};
	jni_error_code_to_result(res)?;

	// Increment the attached thread count only if requested.
	//
	// This is used to avoid double-counting when re-attaching a permanently-attached thread that
	// got manually detached outside of this crate's APIs (e.g., via raw JNI calls).
	//
	// I.e. the only time this should be false is when we are re-attaching via
	// tls_attach_current_thread or fls_attach_current_thread after detecting that the thread
	// already had a permanent attachment.
	if inc_attached_count {
		ATTACHED_THREADS.fetch_add(1, Ordering::SeqCst);
	}

	debug!(
		"Attached thread {} ({:?}). {} threads attached",
		thread.name().unwrap_or_default(),
		thread.id(),
		ATTACHED_THREADS.load(Ordering::SeqCst)
	);

	Ok(env_ptr.cast::<sys::JNIEnv>())
}

/// Check if the process is currently shutting down.
///
/// Note: RtlDllShutdownInProgress is available on Windows 10 + but not part of
/// the Windows SDK (not available via windows-bindgen), so we link to it
/// manually.
#[cfg(target_os = "windows")]
fn windows_is_shutdown_in_progress() -> bool {
	#[link(name = "ntdll")]
	unsafe extern "system" {
		fn RtlDllShutdownInProgress() -> u8; // BOOLEAN
	}
	unsafe { RtlDllShutdownInProgress() != 0 }
}

/// Detach a thread from the JVM.
///
/// If `cross_check_env` is provided, this function will verify that the env pointer matches the
/// current thread's attachment before detaching. This provides a defensive check that we "own" the
/// current attachment. We only use this when detaching scoped attachments.
///
/// `cross_check_env` is not provided for permanent TLS/FLS attachments since we allow external
/// code to detach these early (as long as there are no `AttachGuards` currently on the stack).
///
/// The `ATTACHED_THREADS` count is always decremented when this is called, even if the thread
/// was already detached by other means (e.g., manual `DetachCurrentThread` call via raw JNI API,
/// or by another fiber on Windows). This maintains correct logical accounting - we track how many
/// times *we* attached, not the actual JVM attachment state.
pub unsafe fn sys_detach_current_thread(
	cross_check_env: Option<*mut abos_jni_sys::JNIEnv>,
	thread: &Thread,
) -> Result<()> {
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);

	let vm = JavaVM::singleton()?;

	// On Windows we don't ever attempt to explicitly detach a thread from the JVM (or even query if
	// we have a valid attachment) if we know the process is already shutting down.
	//
	// On Windows, when the process is shutting down there's a risk of deadlocks if we attempt to
	// call into the JVM while the thread holds the system loader lock. This is because the JVM may
	// need to reach a safepoint, which also needs to acquire the system loader lock.
	//
	// In practice we need this check on Windows because we register fiber local storage callbacks
	// that may either be called when a fiber/thread terminates _OR_ when the process is shutting
	// down, and we only really want to auto-detach threads in the former case.
	//
	// Note: we don't want to entirely skip the call to `sys_detach_current_thread` during shutdown
	// because we still want to maintain correct logical bookkeeping.
	//
	// Ref: https://github.com/jni-rs/jni-rs/issues/701
	#[cfg(target_os = "windows")]
	let allow_jni = !windows_is_shutdown_in_progress();
	#[cfg(not(target_os = "windows"))]
	let allow_jni = true;

	if allow_jni {
		// Check if thread is actually attached before attempting to detach
		// SAFETY: This routine runs on the detaching thread and uses the temporary guard only within this scope.
		let was_attached = match unsafe { vm.sys_get_env_attachment() } {
			Ok(mut guard) => {
				// Optional defensive cross-check: verify env matches current thread
				if let Some(expected_env) = cross_check_env {
					// If this check fails it would imply we're holding an
					// invalid pointer and would be completely unsound
					assert_eq!(
						guard.env.raw, expected_env,
						"BUG: Something meddled with the JNI attachment behind our back"
					);
				}

				// Clear Pending Exceptions
				//
				// Unlike for later versions; the JNI spec for Java 8 doesn't explicitly state that
				// DetachCurrentThread is safe to call while there are pending exceptions.
				//
				// Note: we don't use `.exception_catch()` to get the details of any exception since
				// we assume that will be done via `AttachGuard::detach_with_catch` if needed.

				guard.borrow_env_mut().exception_clear();

				drop(guard);
				true
			}
			Err(Error::JniCall(JniError::ThreadDetached)) => {
				// If this assertion fails it implies we have been left with a potentially invalid
				// pointer for an attachment that no longer exists.
				assert!(cross_check_env.is_none(), "BUG: Thread was detached by external code");
				false
			}
			Err(e) => return Err(e),
		};

		if was_attached {
			// Only actually detach if thread is still attached
			// SAFETY: The current thread is attached and the temporary environment guard has already been released.
			unsafe {
				java_vm_call_unchecked!(vm, v1_1, DetachCurrentThread);
			}
			debug!(
				"Detached thread {} ({:?})",
				thread.name().unwrap_or_default(),
				thread.id()
			);
		} else {
			// Thread already detached (e.g., by another fiber on Windows, or manual DetachCurrentThread)
			debug!(
				"Thread {} ({:?}) already detached (skipping DetachCurrentThread)",
				thread.name().unwrap_or_default(),
				thread.id()
			);
		}
	}

	// ALWAYS decrement the logical count, even if thread was already detached.
	// This maintains correct bookkeeping:
	// - On Windows with FLS: each fiber attachment increments this counter
	// - On other platforms with TLS: each thread attachment increments this counter
	ATTACHED_THREADS.fetch_sub(1, Ordering::SeqCst);

	debug!(
		"{} attachments remain (logical count)",
		ATTACHED_THREADS.load(Ordering::SeqCst)
	);

	Ok(())
}

// There's a false-positive Clippy bug: https://github.com/rust-lang/rust-clippy/issues/13422
thread_local! {
	#[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local))]
	static THREAD_ATTACHMENT: Cell<*mut abos_jni_sys::JNIEnv> = const { Cell::new(std::ptr::null_mut()) };
	#[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local))]
	pub(super) static THREAD_GUARD_NEST_LEVEL: Cell<usize> = const { Cell::new(0) };
}

/// A non-Send, non-Sync token, with no `const` constructor, representing a
/// local scope when attaching the current thread to a Java VM.
///
/// This gives us something for an [`AttachGuard`] to borrow that's not likely
/// to be accidentally made `'static`.
///
/// This is only relevant for `unsafe` code that is manually creating
/// [`AttachGuard`]s.
///
/// See: [`JavaVM::attach_current_thread_guard`] and [`JavaVM::get_env_attachment`].
#[derive(Debug, Default)]
pub struct ScopeToken {
	_non_send_sync: std::marker::PhantomData<*const ()>,
}

#[cfg(test)]
mod test {
	use crate::{AttachGuard, ScopeToken};
	static_assertions::assert_not_impl_any!(ScopeToken: Send);
	static_assertions::assert_not_impl_any!(ScopeToken: Sync);

	static_assertions::assert_not_impl_any!(AttachGuard: Send);
	static_assertions::assert_not_impl_any!(AttachGuard: Sync);
}

mod access;
mod attachment;
mod config;
mod guard;
mod invocation;
pub use config::*;
pub use guard::*;
