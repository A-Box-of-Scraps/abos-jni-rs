use core::ffi::c_void;

use crate::{Env, Error, JvmtiVersion, Result, internal::error::check, sys};

pub(super) fn count(length: usize) -> Result<sys::jint> {
	length.try_into().map_err(|_| Error::AllocationTooLarge)
}

impl Env<'_> {
	/// Enables or disables an event for one thread (JVMTI 1.2, or the event's version).
	///
	/// A null thread selects global delivery. Per-thread disabling does not override
	/// global enabling. Events that do not support thread filtering are rejected by
	/// the VM. The event's capability must be held; newer events are version-checked.
	/// No reference is owned by this wrapper.
	///
	/// # Errors
	/// Returns unsupported-version, missing-function, or native errors (including
	/// invalid event/thread and missing capability).
	///
	/// # Safety
	/// Call in `OnLoad` or live phase on a JVMTI-permitted thread, outside restricted
	/// callbacks (not callback-safe). A non-null thread must be a valid JNI reference.
	/// Registered callbacks must be ready for concurrent execution before enabling.
	pub unsafe fn set_thread_event_enabled(
		&self,
		thread: sys::jthread,
		event: sys::jvmtiEvent,
		enabled: bool,
	) -> Result<()> {
		let required: sys::jint =
			sys::jvmti_event_version(event).ok_or(Error::Jvmti(sys::JVMTI_ERROR_INVALID_EVENT_TYPE))?;
		self.ensure_version(JvmtiVersion::from_raw(required)?)?;
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
		// SAFETY: The caller supplies valid context and thread; standard events require no varargs.
		check(unsafe { function(self.as_raw(), mode, event, thread) })
	}

	/// Relinquishes selected capabilities (JVMTI 1.2, or the capability's version).
	///
	/// No ownership is transferred. Reacquisition is not guaranteed.
	///
	/// # Errors
	/// Returns an unsupported-version, missing-function, or native error.
	///
	/// # Safety
	/// Call in `OnLoad` or live phase on a JVMTI-permitted thread, outside restricted
	/// callbacks (not callback-safe). Reserved bits must be zero. Coordinate with
	/// all other users of this environment and disable affected events first.
	pub unsafe fn relinquish_capabilities(&self, capabilities: &sys::jvmtiCapabilities) -> Result<()> {
		self.ensure_version(JvmtiVersion::from_raw(capabilities.required_version())?)?;
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *const sys::jvmtiCapabilities) -> sys::jvmtiError =
			self.table()
				.RelinquishCapabilities
				.ok_or(Error::MissingFunction("RelinquishCapabilities"))?;
		// SAFETY: The caller supplies a valid capability selection and execution context.
		check(unsafe { function(self.as_raw(), capabilities) })
	}

	/// Returns the agent-owned environment storage pointer (JVMTI 1.2).
	///
	/// Initially null. No capability is needed and no ownership is transferred.
	///
	/// # Errors
	/// Returns a native or missing-function error.
	///
	/// # Safety
	/// Any phase, callback-safe including heap/GC/ObjectFree callbacks. The thread
	/// must be JVMTI-permitted. Synchronize storage changes and pointee access with
	/// other environment users; the returned pointer conveys no Rust lifetime.
	pub unsafe fn environment_local_storage(&self) -> Result<*mut c_void> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut *mut c_void) -> sys::jvmtiError = self
			.table()
			.GetEnvironmentLocalStorage
			.ok_or(Error::MissingFunction("GetEnvironmentLocalStorage"))?;
		let mut pointer: *mut c_void = core::ptr::null_mut();
		// SAFETY: Output is writable and caller supplies the context.
		check(unsafe { function(self.as_raw(), &raw mut pointer) })?;
		Ok(pointer)
	}

	/// Replaces the agent-owned environment storage pointer (JVMTI 1.2).
	///
	/// No capability is needed. The VM neither frees the previous pointer nor takes
	/// ownership of the new one; null clears the slot. Clear and synchronize users
	/// before freeing agent state or unloading callback code.
	///
	/// # Errors
	/// Returns a native or missing-function error.
	///
	/// # Safety
	/// The phase, callback, thread, and synchronization requirements of
	/// [`Self::environment_local_storage`] apply. Keep stored state alive for all
	/// readers and ensure that they interpret its type correctly.
	pub unsafe fn set_environment_local_storage(&self, pointer: *mut c_void) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *const c_void) -> sys::jvmtiError = self
			.table()
			.SetEnvironmentLocalStorage
			.ok_or(Error::MissingFunction("SetEnvironmentLocalStorage"))?;
		// SAFETY: Caller manages storage lifetime, synchronization, and context.
		check(unsafe { function(self.as_raw(), pointer) })
	}
}
