use crate::{Env, Error, Result, internal::error::check, sys};

impl Env<'_> {
	/// Installs a breakpoint at a method location (JVMTI 1.2).
	///
	/// Requires `can_generate_breakpoint_events`. Locations use the VM's location
	/// format. No method ownership is transferred. Enable the Breakpoint event
	/// separately; class redefinition clears breakpoints in the redefined class.
	///
	/// # Errors
	/// Returns native errors such as duplicate/invalid locations or missing
	/// capability, or a missing-function error.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The method ID must be valid.
	pub unsafe fn set_breakpoint(&self, method: sys::jmethodID, location: sys::jlocation) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jmethodID, sys::jlocation) -> sys::jvmtiError =
			self.table()
				.SetBreakpoint
				.ok_or(Error::MissingFunction("SetBreakpoint"))?;
		// SAFETY: Caller supplies the valid method and execution context.
		check(unsafe { function(self.as_raw(), method, location) })
	}

	/// Removes a breakpoint (JVMTI 1.2; `can_generate_breakpoint_events`).
	///
	/// # Errors
	/// Returns native errors (including `NOT_FOUND`) or a missing function.
	///
	/// # Safety
	/// The phase, thread, callback, and method-validity requirements of
	/// [`Self::set_breakpoint`] apply. No ownership is transferred.
	pub unsafe fn clear_breakpoint(&self, method: sys::jmethodID, location: sys::jlocation) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jmethodID, sys::jlocation) -> sys::jvmtiError =
			self.table()
				.ClearBreakpoint
				.ok_or(Error::MissingFunction("ClearBreakpoint"))?;
		// SAFETY: Caller supplies the valid method and execution context.
		check(unsafe { function(self.as_raw(), method, location) })
	}

	/// Configures single-step events for a thread (JVMTI 1.2).
	///
	/// Requires `can_generate_single_step_events`. Null selects global delivery;
	/// disabling per-thread stepping does not override global enabling.
	///
	/// # Errors
	/// Returns native or missing-function errors from event configuration.
	///
	/// # Safety
	/// The phase, thread, callback, and ownership requirements of
	/// [`Self::set_thread_event_enabled`] apply.
	pub unsafe fn set_single_step_enabled(&self, thread: sys::jthread, enabled: bool) -> Result<()> {
		// SAFETY: Caller supplies the event configuration context and valid thread.
		unsafe { self.set_thread_event_enabled(thread, sys::JVMTI_EVENT_SINGLE_STEP, enabled) }
	}
}
