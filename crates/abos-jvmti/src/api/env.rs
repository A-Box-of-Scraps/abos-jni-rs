use core::{marker::PhantomData, ptr::NonNull};

use crate::{Error, JvmtiVersion, Result, internal::error::check, sys};

mod construction;
mod debugging;
mod heap;
pub(super) mod instrumentation;
mod locals;
mod management;
mod returned_data;
mod threads;

/// A borrowed JVMTI 1.2-or-newer environment, usable across threads.
///
/// This handle neither attaches threads nor disposes the environment on drop.
/// Copies refer to the same environment, including its capabilities and callbacks.
/// All copies and allocations must stop being used before the environment is
/// disposed or the VM is destroyed.
#[derive(Clone, Copy, Debug)]
pub struct Env<'vm> {
	raw: NonNull<sys::jvmtiEnv>,
	lifetime: PhantomData<&'vm sys::jvmtiEnv>,
	version: JvmtiVersion,
}

// SAFETY: JVMTI environments are cross-thread. Every native call is unsafe and
// requires the caller to satisfy thread attachment and execution-context rules.
unsafe impl Send for Env<'_> {}
// SAFETY: Sharing a handle does not mutate the interface table. The VM synchronizes
// its environment state; agent-owned callback state must be synchronized by callers.
unsafe impl Sync for Env<'_> {}

impl Env<'_> {
	/// Returns the borrowed native environment pointer.
	#[must_use]
	pub const fn as_raw(self) -> *mut sys::jvmtiEnv {
		self.raw.as_ptr()
	}

	pub(crate) fn table(&self) -> &sys::jvmtiInterface_1__1_2 {
		// SAFETY: Construction validates JVMTI >= 1.2. Borrow only the baseline
		// table, which ends before slots appended in later versions.
		unsafe { &*(*self.raw.as_ptr()).cast::<sys::jvmtiInterface_1__1_2>() }
	}

	pub(crate) fn ensure_version(&self, required: JvmtiVersion) -> Result<()> {
		construction::require_version(self.version, required)
	}

	/// Returns the actual interface version retained at construction.
	///
	/// Makes no JVM call and can be used in restricted callbacks.
	#[must_use]
	pub const fn version(&self) -> JvmtiVersion {
		self.version
	}

	/// Returns the current VM phase.
	///
	/// # Errors
	/// Returns the native JVMTI error or a missing-function error.
	///
	/// # Safety
	/// The current thread must satisfy JVMTI's thread-attachment requirements.
	/// This operation is permitted in any phase but is not callback-safe.
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
	/// Returns an unsupported-version error for newer capability bits, or the native
	/// JVMTI error for capabilities unavailable in this VM or phase.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Reserved bits
	/// must be zero. Coordinate capability changes with other users of this environment.
	pub unsafe fn add_capabilities(&self, capabilities: &sys::jvmtiCapabilities) -> Result<()> {
		self.ensure_version(JvmtiVersion::from_raw(capabilities.required_version())?)?;
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
	/// Returns an unsupported-version error for newer callbacks, a native JVMTI
	/// error, or a missing-function error. Unsupported callbacks are not silently dropped.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Reserved slots
	/// must be null. Callbacks must remain executable until no invocation can occur,
	/// be reentrant, obey their JVMTI/JNI context rules, and never unwind into the VM.
	/// Replacing callbacks does not wait for already-running callbacks to finish.
	pub unsafe fn set_event_callbacks(&self, callbacks: &sys::jvmtiEventCallbacks) -> Result<()> {
		self.ensure_version(JvmtiVersion::from_raw(callbacks.required_version())?)?;
		let size: sys::jint = sys::jvmtiEventCallbacks::size_for_version(self.version.as_raw())
			.and_then(|size| size.try_into().ok())
			.ok_or(Error::InvalidVersion(self.version.as_raw()))?;
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			*const sys::jvmtiEventCallbacks,
			sys::jint,
		) -> sys::jvmtiError = self
			.table()
			.SetEventCallbacks
			.ok_or(Error::MissingFunction("SetEventCallbacks"))?;
		// SAFETY: The table is readable for the version-appropriate size; caller upholds callback contracts.
		check(unsafe { function(self.as_raw(), callbacks, size) })
	}

	/// Enables or disables an event globally for this environment.
	///
	/// # Errors
	/// Returns an unsupported-version error for newer events, or a native JVMTI
	/// error for invalid events or missing capabilities.
	///
	/// # Safety
	/// The context requirements of [`Self::capabilities`] apply. Registered callbacks
	/// must be ready to run concurrently before an event is enabled.
	pub unsafe fn set_event_enabled(&self, event: sys::jvmtiEvent, enabled: bool) -> Result<()> {
		// SAFETY: The caller supplies the event context; null selects global delivery.
		unsafe { self.set_thread_event_enabled(core::ptr::null_mut(), event, enabled) }
	}

	/// Disposes the native environment. Dropping an ordinary handle does not do this.
	///
	/// See the [explicit shutdown example](crate#explicit-shutdown-coordination) for
	/// admission control, draining, and cleanup before disposal. It is not a library-unload protocol.
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
