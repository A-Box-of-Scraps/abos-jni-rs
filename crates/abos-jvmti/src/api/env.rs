use core::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use crate::{Error, Result, internal::error::check, sys};

/// A borrowed JVMTI 25 environment, usable across threads.
///
/// This handle neither attaches threads nor disposes the environment on drop.
/// Copies refer to the same environment, including its capabilities and callbacks.
/// All copies and allocations must stop being used before the environment is
/// disposed or the VM is destroyed.
#[derive(Clone, Copy, Debug)]
pub struct Env<'vm> {
	raw: NonNull<sys::jvmtiEnv>,
	lifetime: PhantomData<&'vm sys::jvmtiEnv>,
}

// SAFETY: JVMTI environments are cross-thread. Every native call is unsafe and
// requires the caller to satisfy thread attachment and execution-context rules.
unsafe impl Send for Env<'_> {}
// SAFETY: Sharing a handle does not mutate the interface table. The VM synchronizes
// its environment state; agent-owned callback state must be synchronized by callers.
unsafe impl Sync for Env<'_> {}

impl<'vm> Env<'vm> {
	/// Borrows an existing environment without taking ownership.
	///
	/// # Errors
	/// Returns [`Error::NullPointer`] for a null environment or function table.
	///
	/// # Safety
	/// A non-null pointer must reference an initialized JVMTI 25 environment and
	/// its full function table. Both must remain valid and the table immutable
	/// for `'vm`, even when handles are sent to other threads. The environment
	/// must not be disposed and the VM must not be destroyed during that lifetime.
	pub unsafe fn from_raw(raw: *mut sys::jvmtiEnv) -> Result<Self> {
		let raw: NonNull<sys::jvmtiEnv> = NonNull::new(raw).ok_or(Error::NullPointer)?;
		// SAFETY: The caller guarantees that the environment pointer is readable.
		if unsafe { *raw.as_ptr() }.is_null() {
			return Err(Error::NullPointer);
		}
		Ok(Self {
			raw,
			lifetime: PhantomData,
		})
	}

	/// Creates an independent JVMTI 25 environment using an existing JNI VM.
	///
	/// # Errors
	/// Returns a JNI version/availability error or a null-interface error.
	///
	/// # Safety
	/// The VM must remain alive for `'vm`. Call only in the `OnLoad` or live phase,
	/// on a thread permitted to call `GetEnv`. The returned environment must not
	/// be disposed while any borrowed handle or allocation is in use.
	pub unsafe fn from_vm(vm: &'vm abos_jni::JavaVM) -> Result<Self> {
		// SAFETY: JavaVM guarantees its pointer's layout; remaining rules are the caller's.
		unsafe { Self::from_raw_vm(vm.get_raw()) }
	}

	/// Creates an independent JVMTI 25 environment from an agent's raw VM pointer.
	///
	/// The environment is not automatically disposed, allowing callbacks to use
	/// it after `Agent_OnLoad` returns. Use [`Self::dispose`] when appropriate.
	///
	/// # Errors
	/// Returns the `GetEnv` JNI error or [`Error::NullPointer`].
	///
	/// # Safety
	/// The VM pointer must be valid and support JNI 1.2 or newer. The VM must
	/// remain alive for `'vm`. The phase, attachment, and environment lifetime
	/// requirements of [`Self::from_vm`] also apply.
	pub unsafe fn from_raw_vm(vm: *mut sys::JavaVM) -> Result<Self> {
		if vm.is_null() {
			return Err(Error::NullPointer);
		}
		// SAFETY: The caller guarantees a readable VM pointer and JNI 1.2 table.
		let table: *const sys::JNIInvokeInterface_ = unsafe { *vm };
		if table.is_null() {
			return Err(Error::NullPointer);
		}
		let mut raw: *mut c_void = core::ptr::null_mut();
		// SAFETY: GetEnv is in the JNI 1.2 view; the output pointer is writable.
		let code: sys::jint = unsafe { ((*table).v1_2.GetEnv)(vm, &raw mut raw, sys::JVMTI_VERSION) };
		if code != sys::JNI_OK {
			return Err(Error::Jni(code));
		}
		// SAFETY: A successful JVMTI 25 GetEnv returns the requested interface.
		unsafe { Self::from_raw(raw.cast()) }
	}

	/// Returns the borrowed native environment pointer.
	#[must_use]
	pub const fn as_raw(self) -> *mut sys::jvmtiEnv {
		self.raw.as_ptr()
	}

	pub(crate) fn table(&self) -> &sys::jvmtiInterface_1_ {
		// SAFETY: Construction requires a live immutable full table for the handle lifetime.
		unsafe { &**self.raw.as_ptr() }
	}

	/// Returns the environment's interface version.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The current thread must satisfy JVMTI's thread-attachment requirements.
	/// This operation is callback-safe and permitted in any phase.
	pub unsafe fn version(&self) -> Result<sys::jint> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jint) -> sys::jvmtiError = self
			.table()
			.GetVersionNumber
			.ok_or(Error::MissingFunction("GetVersionNumber"))?;
		let mut value: sys::jint = 0;
		// SAFETY: The context is guaranteed by the caller; value is writable.
		check(unsafe { function(self.as_raw(), &raw mut value) })?;
		Ok(value)
	}

	/// Returns the current VM phase.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The current thread must satisfy JVMTI's thread-attachment requirements.
	/// This operation is callback-safe and permitted in any phase.
	pub unsafe fn phase(&self) -> Result<sys::jvmtiPhase> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jvmtiPhase) -> sys::jvmtiError =
			self.table().GetPhase.ok_or(Error::MissingFunction("GetPhase"))?;
		let mut value: sys::jvmtiPhase = 0;
		// SAFETY: The context is guaranteed by the caller; value is writable.
		check(unsafe { function(self.as_raw(), &raw mut value) })?;
		Ok(value)
	}

	/// Returns capabilities currently held by this environment.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// Call in the `OnLoad` or live phase on a JVMTI-permitted thread, outside
	/// heap-iteration callbacks and GC/ObjectFree handlers (not callback-safe).
	pub unsafe fn capabilities(&self) -> Result<sys::jvmtiCapabilities> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jvmtiCapabilities) -> sys::jvmtiError =
			self.table()
				.GetCapabilities
				.ok_or(Error::MissingFunction("GetCapabilities"))?;
		let mut value: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		// SAFETY: The caller supplies the context; value is a writable capability record.
		check(unsafe { function(self.as_raw(), &raw mut value) })?;
		Ok(value)
	}

	/// Returns capabilities that this environment could acquire in the current phase.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply.
	pub unsafe fn potential_capabilities(&self) -> Result<sys::jvmtiCapabilities> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jvmtiCapabilities) -> sys::jvmtiError =
			self.table()
				.GetPotentialCapabilities
				.ok_or(Error::MissingFunction("GetPotentialCapabilities"))?;
		let mut value: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		// SAFETY: The caller supplies the context; value is a writable capability record.
		check(unsafe { function(self.as_raw(), &raw mut value) })?;
		Ok(value)
	}

	/// Acquires the requested capabilities for this environment.
	///
	/// # Errors
	/// Returns the native JVMTI error, including unavailable capabilities.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Reserved bits
	/// must be zero. Coordinate capability changes with other users of this environment.
	pub unsafe fn add_capabilities(&self, capabilities: &sys::jvmtiCapabilities) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *const sys::jvmtiCapabilities) -> sys::jvmtiError =
			self.table()
				.AddCapabilities
				.ok_or(Error::MissingFunction("AddCapabilities"))?;
		// SAFETY: The caller supplies the context and a valid capability selection.
		check(unsafe { function(self.as_raw(), capabilities) })
	}

	/// Replaces all event callbacks. The VM copies the table before returning.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Reserved slots
	/// must be null. Callbacks must remain executable until no invocation can occur,
	/// be reentrant, obey their JVMTI/JNI context rules, and never unwind into the VM.
	/// Replacing callbacks does not wait for already-running callbacks to finish.
	pub unsafe fn set_event_callbacks(&self, callbacks: &sys::jvmtiEventCallbacks) -> Result<()> {
		#[allow(
			clippy::cast_possible_truncation,
			clippy::cast_possible_wrap,
			reason = "A 39-pointer callback table fits jint on supported targets."
		)]
		const SIZE: sys::jint = size_of::<sys::jvmtiEventCallbacks>() as sys::jint;
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			*const sys::jvmtiEventCallbacks,
			sys::jint,
		) -> sys::jvmtiError = self
			.table()
			.SetEventCallbacks
			.ok_or(Error::MissingFunction("SetEventCallbacks"))?;
		// SAFETY: The table is readable for SIZE bytes; caller upholds callback contracts.
		check(unsafe { function(self.as_raw(), callbacks, SIZE) })
	}

	/// Enables or disables an event globally for this environment.
	///
	/// # Errors
	/// Returns the native JVMTI error, including invalid events or missing capabilities.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Registered callbacks
	/// must be ready to run concurrently before an event is enabled.
	pub unsafe fn set_event_enabled(&self, event: sys::jvmtiEvent, enabled: bool) -> Result<()> {
		let function: unsafe extern "C" fn(
			*mut sys::jvmtiEnv,
			sys::jvmtiEventMode,
			sys::jvmtiEvent,
			sys::jthread,
			...
		) -> sys::jvmtiError = self
			.table()
			.SetEventNotificationMode
			.ok_or(Error::MissingFunction("SetEventNotificationMode"))?;
		let mode: sys::jvmtiEventMode = if enabled { sys::JVMTI_ENABLE } else { sys::JVMTI_DISABLE };
		// SAFETY: A null thread selects global notification; standard events need no varargs.
		check(unsafe { function(self.as_raw(), mode, event, core::ptr::null_mut()) })
	}

	/// Disposes the native environment. Dropping an ordinary handle does not do this.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. No other handle,
	/// callback, or allocation may subsequently use the environment. Synchronize
	/// with in-flight callbacks first. Release allocations and raw monitors and
	/// resume suspended threads before disposal; disposal does not release them.
	pub unsafe fn dispose(self) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv) -> sys::jvmtiError = self
			.table()
			.DisposeEnvironment
			.ok_or(Error::MissingFunction("DisposeEnvironment"))?;
		// SAFETY: The caller guarantees exclusive disposal and the required VM context.
		check(unsafe { function(self.as_raw()) })
	}
}
