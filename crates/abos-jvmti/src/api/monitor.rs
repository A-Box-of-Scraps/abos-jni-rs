use core::{marker::PhantomData, mem::ManuallyDrop, ptr};

use abos_jni::strings::JNIStr;

use crate::{Env, Error, Result, internal::error::check, sys};

type MonitorFunction = unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jrawMonitorID) -> sys::jvmtiError;

/// An exclusively owned JVMTI raw monitor, shareable across attached threads.
///
/// Raw monitors are recursive locks, independent of Java object monitors. This
/// type does not protect Rust data automatically. All native operations require
/// an unsafe context contract; a guard only tracks a single recursive entry.
///
/// Keep the environment and VM live until destruction, in `OnLoad` or live phase on
/// a JVMTI-permitted thread. Creation/destruction and locking are callback-safe,
/// including heap/GC/ObjectFree callbacks. Do not destroy while any other user
/// owns, waits for, or may access this monitor. Drop attempts destruction once and
/// ignores errors; [`Self::destroy`] reports errors, without retrying on failure.
/// Failed destruction can leak. Use [`Self::into_raw`] for manual recovery.
#[derive(Debug)]
pub struct RawMonitor<'env, 'vm> {
	env: &'env Env<'vm>,
	raw: sys::jrawMonitorID,
	destroy: MonitorFunction,
}

// SAFETY: JVMTI raw monitors are cross-thread; every native operation is unsafe
// and destruction requires the caller to exclude all outstanding native users.
unsafe impl Send for RawMonitor<'_, '_> {}
// SAFETY: Shared monitor operations are synchronized by the VM. Rust data access
// is not exposed by this type and remains the caller's responsibility.
unsafe impl Sync for RawMonitor<'_, '_> {}

impl Env<'_> {
	/// Creates an owned recursive raw monitor (JVMTI 1.2; no capability).
	///
	/// The VM copies the modified UTF-8 name before returning.
	///
	/// # Errors
	/// Returns native creation errors, a missing creation/destruction function, or
	/// a null successful output.
	///
	/// # Safety
	/// Call in `OnLoad` or live phase on a JVMTI-permitted thread. Callback-safe,
	/// including restricted callbacks. Keep the environment live and satisfy
	/// [`RawMonitor`]'s destruction-context and exclusive-ownership requirements.
	pub unsafe fn create_raw_monitor(&self, name: &JNIStr) -> Result<RawMonitor<'_, '_>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			*const core::ffi::c_char,
			*mut sys::jrawMonitorID,
		) -> sys::jvmtiError = self
			.table()
			.CreateRawMonitor
			.ok_or(Error::MissingFunction("CreateRawMonitor"))?;
		let destroy: MonitorFunction = self
			.table()
			.DestroyRawMonitor
			.ok_or(Error::MissingFunction("DestroyRawMonitor"))?;
		let mut raw: sys::jrawMonitorID = ptr::null_mut();
		// SAFETY: Caller supplies context; name is terminated and the output is writable.
		check(unsafe { function(self.as_raw(), name.as_ptr(), &raw mut raw) })?;
		if raw.is_null() {
			return Err(Error::NullPointer);
		}
		Ok(RawMonitor {
			env: self,
			raw,
			destroy,
		})
	}
}

impl<'env, 'vm> RawMonitor<'env, 'vm> {
	/// Adopts a monitor created by this environment (JVMTI 1.2).
	///
	/// No VM call is made; on error ownership stays with the caller.
	///
	/// # Errors
	/// Returns a null-pointer or missing-destruction-function error.
	///
	/// # Safety
	/// `raw` must be a live, exclusively owned monitor from this environment. All
	/// outstanding native users must be coordinated with this owner. The environment,
	/// thread, phase, and cleanup rules in [`RawMonitor`] apply.
	pub unsafe fn from_raw(env: &'env Env<'vm>, raw: sys::jrawMonitorID) -> Result<Self> {
		if raw.is_null() {
			return Err(Error::NullPointer);
		}
		let destroy: MonitorFunction = env
			.table()
			.DestroyRawMonitor
			.ok_or(Error::MissingFunction("DestroyRawMonitor"))?;
		Ok(Self { env, raw, destroy })
	}

	/// Borrows the native monitor ID without transferring ownership.
	#[must_use]
	pub const fn as_raw(&self) -> sys::jrawMonitorID {
		self.raw
	}

	/// Transfers ownership, leaving destruction to the caller. Makes no VM call.
	#[must_use]
	pub fn into_raw(self) -> sys::jrawMonitorID {
		ManuallyDrop::new(self).raw
	}

	/// Destroys this monitor once, without retrying in drop (JVMTI 1.2).
	///
	/// # Errors
	/// Returns the native destruction error; a failed destruction may leak.
	///
	/// # Safety
	/// `OnLoad` or live phase, callback-safe, no capability. The caller must be on a
	/// JVMTI-permitted thread, with no other owner, waiter, or future native user.
	/// The environment must be live. Destruction transfers no JNI ownership.
	pub unsafe fn destroy(mut self) -> Result<()> {
		self.release()
	}

	fn release(&mut self) -> Result<()> {
		let raw: sys::jrawMonitorID = core::mem::replace(&mut self.raw, ptr::null_mut());
		if raw.is_null() {
			return Ok(());
		}
		// SAFETY: Creation/adoption requires the destruction context through cleanup;
		// ownership was removed before the call so drop never retries a failed destroy.
		check(unsafe { (self.destroy)(self.env.as_raw(), raw) })
	}

	/// Enters this recursive monitor and owns one matching exit (JVMTI 1.2).
	///
	/// # Errors
	/// Returns a missing enter/exit function or the native locking error.
	///
	/// # Safety
	/// Any phase, callback-safe, no capability. The thread must be JVMTI-permitted
	/// and remain so until guard cleanup. Avoid deadlocks, particularly in restricted
	/// callbacks. The monitor/environment must remain live. Synchronization of Rust
	/// data, external monitor entries, and recursive waits is the caller's responsibility.
	pub unsafe fn enter(&self) -> Result<RawMonitorGuard<'_, 'env, 'vm>> {
		let function: MonitorFunction = self
			.env
			.table()
			.RawMonitorEnter
			.ok_or(Error::MissingFunction("RawMonitorEnter"))?;
		let exit: MonitorFunction = self
			.env
			.table()
			.RawMonitorExit
			.ok_or(Error::MissingFunction("RawMonitorExit"))?;
		// SAFETY: Caller guarantees valid locking context and native lifetime.
		check(unsafe { function(self.env.as_raw(), self.raw) })?;
		Ok(RawMonitorGuard {
			monitor: self,
			exit,
			active: true,
			thread: PhantomData,
		})
	}
}

impl Drop for RawMonitor<'_, '_> {
	fn drop(&mut self) {
		let _cleanup: Result<()> = self.release();
	}
}

/// One recursive raw-monitor entry, confined to the thread that acquired it.
///
/// Drop attempts one exit and ignores errors. [`Self::unlock`] reports errors and
/// does not retry on failure. The borrowed monitor cannot be destroyed through
/// its owner while this guard exists. It does not provide access to protected Rust
/// data; waiting temporarily releases all recursive entries of the monitor.
#[derive(Debug)]
pub struct RawMonitorGuard<'monitor, 'env, 'vm> {
	monitor: &'monitor RawMonitor<'env, 'vm>,
	exit: MonitorFunction,
	active: bool,
	thread: PhantomData<*mut ()>,
}

impl RawMonitorGuard<'_, '_, '_> {
	/// Releases this entry once (JVMTI 1.2; no capability, any phase, callback-safe).
	///
	/// Uses the acquiring thread and the cleanup context promised to
	/// [`RawMonitor::enter`]. No ownership other than the lock entry is released.
	///
	/// # Errors
	/// Returns the native exit error; failed exits are not retried by drop.
	pub fn unlock(mut self) -> Result<()> {
		self.release()
	}

	fn release(&mut self) -> Result<()> {
		if !core::mem::replace(&mut self.active, false) {
			return Ok(());
		}
		// SAFETY: This non-Send guard owns one entry on the acquiring thread.
		check(unsafe { (self.exit)(self.monitor.env.as_raw(), self.monitor.raw) })
	}

	/// Releases the monitor while waiting, then reacquires it (JVMTI 1.2).
	///
	/// Zero milliseconds waits indefinitely. Notifications may be spurious; check
	/// the condition in a loop. All recursive entries are released while waiting.
	///
	/// # Errors
	/// Returns a missing function or native error, including interruption.
	///
	/// # Safety
	/// Any phase, callback-safe, no capability. The acquiring thread must be
	/// JVMTI-permitted and still own the monitor. Do not hold Rust references whose
	/// validity relies on uninterrupted monitor ownership. Arrange notifications or
	/// a timeout without depending on Java execution stalled by this callback.
	/// No allocation or JNI ownership is transferred.
	pub unsafe fn wait(&self, milliseconds: sys::jlong) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jrawMonitorID, sys::jlong) -> sys::jvmtiError =
			self.monitor
				.env
				.table()
				.RawMonitorWait
				.ok_or(Error::MissingFunction("RawMonitorWait"))?;
		// SAFETY: Caller guarantees ownership, context, and validity across the unlocked interval.
		check(unsafe { function(self.monitor.env.as_raw(), self.monitor.raw, milliseconds) })
	}

	/// Notifies one waiter (JVMTI 1.2; no capability, any phase, callback-safe).
	///
	/// # Errors
	/// Returns a missing function or native monitor error.
	///
	/// # Safety
	/// The acquiring thread must be JVMTI-permitted and still own the monitor.
	/// No JNI or allocation ownership is transferred.
	pub unsafe fn notify(&self) -> Result<()> {
		let function: MonitorFunction = self
			.monitor
			.env
			.table()
			.RawMonitorNotify
			.ok_or(Error::MissingFunction("RawMonitorNotify"))?;
		// SAFETY: Caller guarantees current-thread ownership and the required context.
		check(unsafe { function(self.monitor.env.as_raw(), self.monitor.raw) })
	}

	/// Notifies all waiters (JVMTI 1.2; no capability, any phase, callback-safe).
	///
	/// # Errors
	/// Returns a missing function or native monitor error.
	///
	/// # Safety
	/// The thread and ownership requirements of [`Self::notify`] apply.
	pub unsafe fn notify_all(&self) -> Result<()> {
		let function: MonitorFunction = self
			.monitor
			.env
			.table()
			.RawMonitorNotifyAll
			.ok_or(Error::MissingFunction("RawMonitorNotifyAll"))?;
		// SAFETY: Caller guarantees current-thread ownership and the required context.
		check(unsafe { function(self.monitor.env.as_raw(), self.monitor.raw) })
	}
}

impl Drop for RawMonitorGuard<'_, '_, '_> {
	fn drop(&mut self) {
		let _cleanup: Result<()> = self.release();
	}
}
