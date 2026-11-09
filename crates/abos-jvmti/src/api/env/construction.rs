use core::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use crate::{Env, Error, JvmtiVersion, Result, internal::error::check, sys};

impl<'vm> Env<'vm> {
	/// Borrows an existing environment and queries its actual interface version.
	///
	/// # Errors
	/// Returns a null-interface, version-query, or unsupported-version error.
	///
	/// # Safety
	/// A non-null pointer must reference an initialized JVMTI environment with a
	/// version-appropriate immutable function table. Both must remain valid for
	/// `'vm`, including across threads. Do not dispose the environment or destroy
	/// the VM during that lifetime. The current context must permit
	/// `GetVersionNumber` (any phase, but not callback-safe).
	/// Use [`Self::from_raw_with_version`] when querying is not permitted.
	pub unsafe fn from_raw(raw: *mut sys::jvmtiEnv) -> Result<Self> {
		// SAFETY: The caller guarantees a readable environment and valid table.
		let table: *const sys::jvmtiInterface_1__1_2 = unsafe { checked_table(raw)? };
		// SAFETY: Slot 88 exists in every JVMTI version. Read only that field;
		// do not create a reference to a potentially longer historical table.
		let get_version: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jint) -> sys::jvmtiError =
			unsafe { (*table).GetVersionNumber }.ok_or(Error::MissingFunction("GetVersionNumber"))?;
		let mut actual: sys::jint = 0;
		// SAFETY: The caller permits the version query; the output is writable.
		check(unsafe { get_version(raw, &raw mut actual) })?;
		// SAFETY: The version came from this environment, whose lifetime is guaranteed.
		unsafe { Self::from_raw_with_version(raw, JvmtiVersion::from_raw(actual)?) }
	}

	/// Borrows an environment using its previously queried actual version.
	///
	/// This constructor makes no JVM calls, so it can be used in restricted
	/// callbacks with a version retained during agent initialization.
	///
	/// # Errors
	/// Returns a null-interface or unsupported-version error.
	///
	/// # Safety
	/// `version` must be the actual version of this environment, not merely the
	/// version originally requested from `GetEnv`. The pointer, function table,
	/// and lifetime requirements of [`Self::from_raw`] apply.
	pub unsafe fn from_raw_with_version(raw: *mut sys::jvmtiEnv, version: JvmtiVersion) -> Result<Self> {
		require_version(version, JvmtiVersion::MINIMUM)?;
		// SAFETY: The caller guarantees the environment and table are readable.
		unsafe { checked_table(raw)? };
		Ok(Self {
			raw: NonNull::new(raw).ok_or(Error::NullPointer)?,
			lifetime: PhantomData,
			version,
		})
	}

	/// Creates an independent environment, requesting the JVMTI 1.2 baseline.
	///
	/// # Errors
	/// Returns a JNI invocation, version-query, or invalid-interface error.
	///
	/// # Safety
	/// The VM must remain alive for `'vm`. Call only in the `OnLoad` or live
	/// phase on a thread permitted to call `GetEnv` and `GetVersionNumber`.
	/// Do not dispose the environment while any handle or allocation is in use.
	pub unsafe fn from_vm(vm: &'vm abos_jni::JavaVM) -> Result<Self> {
		// SAFETY: The caller guarantees the VM lifetime and creation context.
		unsafe { Self::from_vm_with_version(vm, JvmtiVersion::MINIMUM) }
	}

	/// Creates an independent environment with an explicit minimum version.
	///
	/// No fallback or second successful `GetEnv` call is performed. The returned
	/// environment may support a newer version, exposed by [`Self::version`].
	///
	/// # Errors
	/// Returns an unsupported-version, JNI invocation, version-query, or
	/// invalid-interface error. Newly created environments are disposed on
	/// validation failure when their disposal function is available.
	///
	/// # Safety
	/// The requirements of [`Self::from_vm`] apply.
	pub unsafe fn from_vm_with_version(vm: &'vm abos_jni::JavaVM, version: JvmtiVersion) -> Result<Self> {
		// SAFETY: JavaVM provides a valid invocation interface; caller supplies the context.
		unsafe { Self::from_raw_vm_with_version(vm.get_raw(), version) }
	}

	/// Creates an independent environment from an agent's VM pointer.
	///
	/// Requests JVMTI 1.2. The environment is not automatically disposed, so
	/// callbacks can use it after `Agent_OnLoad` returns.
	///
	/// # Errors
	/// Returns a JNI invocation, version-query, or invalid-interface error.
	///
	/// # Safety
	/// The pointer must reference a valid VM supporting JNI 1.2 or newer.
	/// The context and lifetime requirements of [`Self::from_vm`] apply.
	pub unsafe fn from_raw_vm(vm: *mut sys::JavaVM) -> Result<Self> {
		// SAFETY: The caller supplies the VM pointer, lifetime, and creation context.
		unsafe { Self::from_raw_vm_with_version(vm, JvmtiVersion::MINIMUM) }
	}

	/// Creates an independent environment with an explicit minimum version.
	///
	/// Each successful `GetEnv` creates a new connection; this does not upgrade
	/// an existing environment or transfer its capabilities and callbacks.
	///
	/// # Errors
	/// The errors and failure-cleanup behavior of [`Self::from_vm_with_version`] apply.
	///
	/// # Safety
	/// The requirements of [`Self::from_raw_vm`] apply.
	pub unsafe fn from_raw_vm_with_version(vm: *mut sys::JavaVM, version: JvmtiVersion) -> Result<Self> {
		require_version(version, JvmtiVersion::MINIMUM)?;
		if vm.is_null() {
			return Err(Error::NullPointer);
		}
		// SAFETY: The caller guarantees a readable VM pointer and JNI 1.2 table.
		let table: *const sys::JNIInvokeInterface_ = unsafe { *vm };
		if table.is_null() {
			return Err(Error::NullPointer);
		}
		let mut raw: *mut c_void = core::ptr::null_mut();
		// SAFETY: GetEnv exists in JNI 1.2 and the output is writable.
		let code: sys::jint = unsafe { ((*table).v1_2.GetEnv)(vm, &raw mut raw, version.as_raw()) };
		if code != sys::JNI_OK {
			return Err(Error::Jni(code));
		}
		let raw: *mut sys::jvmtiEnv = raw.cast();
		// SAFETY: A successful JVMTI GetEnv provides a valid independent environment.
		let result: Result<Self> = unsafe { Self::from_raw(raw) }.and_then(|env| {
			env.ensure_version(version)?;
			Ok(env)
		});
		if result.is_err() {
			// SAFETY: This newly created environment has no other users. Dispose's
			// slot exists even in versions older than the supported baseline.
			if let Ok(table) = unsafe { checked_table(raw) }
                // SAFETY: Read only the disposal slot, not the entire table.
                && let Some(dispose) = unsafe { (*table).DisposeEnvironment }
			{
				// SAFETY: Creation is restricted to phases that permit disposal.
				let _cleanup: sys::jvmtiError = unsafe { dispose(raw) };
			}
		}
		result
	}
}

unsafe fn checked_table(raw: *mut sys::jvmtiEnv) -> Result<*const sys::jvmtiInterface_1__1_2> {
	if raw.is_null() {
		return Err(Error::NullPointer);
	}
	// SAFETY: The caller guarantees that the non-null environment pointer is readable.
	let table: *const sys::jvmtiInterface_1_ = unsafe { *raw };
	if table.is_null() {
		Err(Error::NullPointer)
	} else {
		Ok(table.cast())
	}
}

pub(super) fn require_version(actual: JvmtiVersion, required: JvmtiVersion) -> Result<()> {
	if actual < required {
		Err(Error::UnsupportedVersion { required, actual })
	} else {
		Ok(())
	}
}
