#[cfg(doc)]
use crate::{api::env, objects::JObject, vm::AttachGuard};

use super::{Env, Error, JAVA_VM_SINGLETON, JavaVM, Result, jni_error_code_to_result, ptr, sys};
#[cfg(all(feature = "invocation", not(target_os = "android"), use_fls_attach_guard))]
use crate::internal::fls_attach_guard;
#[cfg(all(feature = "invocation", not(target_os = "android"), use_tls_attach_guard))]
use crate::internal::tls_attach_guard;
#[cfg(all(feature = "invocation", not(target_os = "android")))]
use std::{os::raw::c_void, path::PathBuf};
#[cfg(feature = "invocation")]
use {
	crate::{
		InitArgs,
		errors::{StartJvmError, StartJvmResult},
	},
	std::ffi::OsStr,
};

impl JavaVM {
	/// Launch a new `JavaVM` using the provided init args.
	///
	/// Unlike original JNI API, the main thread (the thread from which this method is called) will
	/// not be attached to JVM. You must explicitly use `attach_current_thread…` methods (refer
	/// to [Attaching Native Threads section](#attaching-native-threads)).
	///
	/// *This API requires the "invocation" feature to be enabled,
	/// see [Launching JVM from Rust](struct.JavaVM.html#launching-jvm-from-rust).*
	///
	/// This will attempt to locate a JVM using
	/// [java-locator], if the JVM has not already been loaded. Use the
	/// [`with_libjvm`][Self::with_libjvm] method to give an explicit location for the JVM shared
	/// library (`jvm.dll`, `libjvm.so`, or `libjvm.dylib`, depending on the platform).
	#[cfg(feature = "invocation")]
	///
	/// # Errors
	///
	/// Returns an error if the JVM library cannot be located or loaded, VM creation fails, or attachment tracking fails.
	pub fn new(args: InitArgs) -> StartJvmResult<Self> {
		#[cfg(not(target_os = "android"))]
		{
			Self::with_libjvm(args, || {
				Ok([
					java_locator::locate_jvm_dyn_library()
						.map_err(StartJvmError::NotFound)?
						.as_str(),
					java_locator::get_jvm_dyn_lib_file_name(),
				]
				.iter()
				.collect::<PathBuf>())
			})
		}

		#[cfg(target_os = "android")]
		{
			drop(args);
			Err(StartJvmError::Unsupported)
		}
	}

	/// Get a [`JavaVM`] for the global Java VM
	///
	/// If no [`JavaVM`] has been initialized, this will return [`Error::UninitializedJavaVM`].
	///
	/// If a [`JavaVM`] has previously been created, either via [`JavaVM::new()`] or
	/// [`JavaVM::from_raw`] then that [`JavaVM`] will be accessible as a global singleton.
	///
	/// This is possible because JNI does not support fully destroying a Java VM and then
	/// initializing a new one and so as soon as we have seen a Java VM pointer once, we know it's
	/// the only VM that will ever exist and it will always be valid in safe code.
	///
	/// If your code observes a [`Env`] reference or an [`AttachGuard`] (from this crate version)
	/// then you can assume that [`JavaVM::singleton()`] has been initialized.
	///
	/// Beware that the observation of reference types (such as [`crate::objects::JObject`]) only
	/// imply that [`JavaVM::singleton()`] has been initialized if the references are non-null.
	///
	/// One other caveat is that native methods may capture reference type arguments, such as
	/// [`JObject`], where their lifetime is _not_ tied to a real `Env`. (And so at the start of
	/// a native method, [`JavaVM::singleton()`] may not be initialized even though we can observe
	/// reference types).
	///
	/// In practice though, you can usually assume [`JavaVM::singleton()`] has been initialized
	/// if you observe non-null reference types, based on the assumption that:
	///
	/// - Before any other `jni-rs` API is used, a native method is expected to use
	///   [`env::EnvUnowned::with_env`] to get a `Env` reference, which will initialize
	///   [`JavaVM::singleton()`].
	/// - For any native method implementation to be safe, it must use `catch_unwind` (e.g. via
	///   [`env::EnvUnowned::with_env`]) to ensure that panics can't unwind over an FFI boundary
	///   (at least rendering an early miss-use of `JavaVM::singleton()` "safe").
	///
	/// Note: that other versions of `jni-rs` within the same application aren't able to share this
	/// singleton state. So you should not make assumptions about this being initialized as a side
	/// effect of other dependencies using `jni-rs` (unless you are using a re-exported version of
	/// `jni-rs` from that dependency). For example the `android-activity` crate will initialize a
	/// [`JavaVM`] before `android_main()` is called, but unless you are using the same version of
	/// `jni-rs` as `android-activity` you can't immediately assume there is a [`JavaVM`] singleton.
	///
	/// # Errors
	///
	/// Returns `Error::UninitializedJavaVM` if no VM has been registered.
	pub fn singleton() -> Result<Self> {
		JAVA_VM_SINGLETON.get().cloned().ok_or(Error::UninitializedJavaVM)
	}

	#[cfg(all(feature = "invocation", not(target_os = "android")))]
	pub(super) unsafe fn with_create_fn_ptr(
		args: InitArgs,
		create_fn_ptr: unsafe extern "system" fn(
			pvm: *mut *mut sys::JavaVM,
			penv: *mut *mut c_void,
			args: *mut c_void,
		) -> sys::jint,
	) -> Result<Self> {
		let mut ptr: *mut sys::JavaVM = ::std::ptr::null_mut();
		let mut env: *mut sys::JNIEnv = ::std::ptr::null_mut();

		// SAFETY: The caller supplies JNI_CreateJavaVM with its declared ABI; output pointers and initialization options are live.
		unsafe {
			jni_error_code_to_result(create_fn_ptr(
				&raw mut ptr,
				(&raw mut env).cast::<*mut c_void>(),
				args.inner_ptr(),
			))?;
		}

		// VM creation has finished reading the option buffers.
		drop(args);

		// SAFETY: Successful JNI_CreateJavaVM initialized the VM pointer.
		let vm = unsafe { Self::from_raw(ptr) };

		// JNI_CreateJavaVM will implicitly attach the calling thread to the JVM.
		//
		// Since the JVM may attribute this thread with special significance as a "main" thread, we
		// avoid detaching it.
		//
		// Instead we take ownership of that attachment by creating a `TLSAttachGuard` for it.
		//
		// Note: This will make a redundant `AttachCurrentThread` call via
		// `sys_attach_current_thread` and the `Default` `config` is benign here because it will be
		// ignored while the thread is already attached.
		//
		// This will track the new attachment in TLS/FLS
		#[cfg(use_tls_attach_guard)]
		// SAFETY: JNI_CreateJavaVM has attached this thread; TLS tracking takes ownership without detaching it.
		unsafe {
			tls_attach_guard::tls_attach_current_thread(&vm, &crate::vm::AttachConfig::default())?
		};
		#[cfg(use_fls_attach_guard)]
		// SAFETY: JNI_CreateJavaVM has attached this thread; FLS tracking takes ownership without detaching it.
		unsafe {
			fls_attach_guard::fls_attach_current_thread(&vm, &crate::vm::AttachConfig::default())?
		};

		Ok(vm)
	}

	/// Launch a new `JavaVM` using the provided init args, loading it from the given shared library file if it's not already loaded.
	///
	/// Unlike original JNI API, the main thread (the thread from which this method is called) will
	/// not be attached to JVM. You must explicitly use `attach_current_thread…` methods (refer
	/// to [Attaching Native Threads section](#attaching-native-threads)).
	///
	/// *This API requires the "invocation" feature to be enabled,
	/// see [Launching JVM from Rust](struct.JavaVM.html#launching-jvm-from-rust).*
	///
	/// The `libjvm_path` parameter takes a *closure* which returns the path to the JVM shared
	/// library. The closure is only called if the JVM is not already loaded. Any work that needs
	/// to be done to locate the JVM shared library should be done inside that closure.
	#[cfg(all(feature = "invocation", not(target_os = "android")))]
	pub(super) fn impl_with_libjvm<P: AsRef<OsStr>>(
		args: InitArgs,
		libjvm_path: impl FnOnce() -> StartJvmResult<P>,
	) -> StartJvmResult<Self> {
		if let Some(jvm) = JAVA_VM_SINGLETON.get() {
			Ok(jvm.clone())
		} else {
			// Determine the path to the shared library.
			let libjvm_path = libjvm_path()?;
			let libjvm_path_string = libjvm_path.as_ref().to_string_lossy().into_owned();

			// Try to load it.
			// SAFETY: This API loads a JNI VM implementation; the library is retained for the lifetime of the created VM.
			let libjvm = match unsafe { libloading::Library::new(libjvm_path.as_ref()) } {
				Ok(ok) => ok,
				Err(error) => return Err(StartJvmError::LoadError(libjvm_path_string, error)),
			};

			// SAFETY: The loaded JVM library exports JNI_CreateJavaVM with the expected ABI and remains loaded for VM use.
			let result = unsafe {
				// Try to find the `JNI_CreateJavaVM` function in the loaded library.
				let create_fn = libjvm
					.get(b"JNI_CreateJavaVM\0")
					.map_err(|error| StartJvmError::LoadError(libjvm_path_string.clone(), error))?;

				// Create the JVM.
				Self::with_create_fn_ptr(args, *create_fn).map_err(StartJvmError::Create)
			};

			if result.is_ok() {
				// Prevent libjvm from being unloaded.
				//
				// If libjvm is unloaded while the JVM is running, the program will crash as soon as it
				// tries to execute any JVM code, including the many threads that the JVM automatically
				// creates.
				//
				// For reasons unknown, HotSpot seems to somehow prevent itself from being unloaded, so it
				// will work even if this `forget` call isn't here, but there's no guarantee that other JVM
				// implementations will also prevent themselves from being unloaded.
				//
				// Note: `jni-rs` makes the assumption that there can only ever be a single `JavaVM`
				// per-process and it's never possible to full destroy and unload a JVM once it's been
				// created. Calling `DestroyJavaVM` is only expected to release some resources and
				// leave the JVM in a poorly-defined limbo state that doesn't allow unloading.
				// Ref: https://github.com/jni-rs/jni-rs/issues/567
				//
				// See discussion at: https://github.com/jni-rs/jni-rs/issues/550
				std::mem::forget(libjvm);
			}

			result
		}
	}

	/// Launch a new `JavaVM` using the provided init args, loading it from the given shared library file if it's not already loaded.
	///
	/// Unlike original JNI API, the main thread (the thread from which this method is called) will
	/// not be attached to JVM. You must explicitly use `attach_current_thread…` methods (refer
	/// to [Attaching Native Threads section](#attaching-native-threads)).
	///
	/// *This API requires the "invocation" feature to be enabled,
	/// see [Launching JVM from Rust](struct.JavaVM.html#launching-jvm-from-rust).*
	///
	/// The `libjvm_path` parameter takes a *closure* which returns the path to the JVM shared
	/// library. The closure is only called if the JVM is not already loaded. Any work that needs
	/// to be done to locate the JVM shared library should be done inside that closure.
	#[cfg(feature = "invocation")]
	///
	/// # Errors
	///
	/// Returns an error if the specified JVM library cannot be loaded, VM creation fails, or attachment tracking fails.
	pub fn with_libjvm<P: AsRef<OsStr>>(
		args: InitArgs,
		libjvm_path: impl FnOnce() -> StartJvmResult<P>,
	) -> StartJvmResult<Self> {
		#[cfg(not(target_os = "android"))]
		{
			Self::impl_with_libjvm(args, libjvm_path)
		}

		#[cfg(target_os = "android")]
		{
			drop(libjvm_path);
			drop(args);
			Err(StartJvmError::Unsupported)
		}
	}

	/// Create a `JavaVM` from a raw pointer.
	///
	/// # Safety
	///
	/// Expects a valid, non-null `JavaVM` pointer that supports JNI version >= 1.4.
	///
	/// Only does a `null` check.
	///
	/// # Panics
	///
	/// Panics if the raw pointer is null.
	pub unsafe fn from_raw(ptr: *mut sys::JavaVM) -> Self {
		assert!(!ptr.is_null());
		JAVA_VM_SINGLETON.get_or_init(|| Self(ptr)).clone()
	}

	/// Returns underlying [`sys::JavaVM`] interface.
	#[must_use]
	pub const fn get_raw(&self) -> *mut sys::JavaVM {
		self.0
	}

	/// Create a `JavaVM` from an Env reference.
	///
	/// This may return [`Error::JavaException`] if called while there is a pending exception.
	pub(crate) fn from_env(env: &Env) -> Result<Self> {
		// Don't use `.get_or_init()` here because it would deadlock if calling `JavaVM::from_raw`
		// which also uses `.get_or_init()`
		if let Some(jvm) = JAVA_VM_SINGLETON.get() {
			Ok(jvm.clone())
		} else {
			let mut raw = ptr::null_mut();
			// SAFETY: The current Env is valid and the output VM pointer is writable.
			let res = unsafe { jni_call_no_post_check_ex!(env, v1_1, GetJavaVM, &raw mut raw)? };
			jni_error_code_to_result(res)?;

			// Safety: The pointer from GetJavaVM should be valid
			unsafe { Ok(Self::from_raw(raw)) }
		}
	}
}
