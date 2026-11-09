use crate::{Env, Error, JvmtiVersion, Result, internal::error::check, sys};

impl Env<'_> {
	/// Copies up to `max_frames` stack frames into Rust-owned storage (JVMTI 1.2).
	///
	/// No capability or suspension is required. Null selects the current thread.
	/// Negative `start_depth` counts from the oldest frame. The vector needs no
	/// JVMTI cleanup, but its method IDs do not keep declaring classes alive.
	///
	/// # Errors
	/// Returns native thread/depth errors, a missing function, or an invalid size/count.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside restricted
	/// callbacks (not callback-safe). A non-null thread must be a valid JNI reference.
	pub unsafe fn stack_trace(
		&self,
		thread: sys::jthread,
		start_depth: sys::jint,
		max_frames: usize,
	) -> Result<Vec<sys::jvmtiFrameInfo>> {
		let maximum: sys::jint = super::management::count(max_frames)?;
		if max_frames > (isize::MAX as usize) / size_of::<sys::jvmtiFrameInfo>() {
			return Err(Error::AllocationTooLarge);
		}
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jvmtiFrameInfo,
			*mut sys::jint,
		) -> sys::jvmtiError = self
			.table()
			.GetStackTrace
			.ok_or(Error::MissingFunction("GetStackTrace"))?;
		let mut frames: Vec<sys::jvmtiFrameInfo> = vec![sys::jvmtiFrameInfo::default(); max_frames];
		let mut count: sys::jint = 0;
		// SAFETY: Buffer is initialized and writable for maximum entries, including a non-null empty buffer.
		check(unsafe {
			function(
				self.as_raw(),
				thread,
				start_depth,
				maximum,
				frames.as_mut_ptr(),
				&raw mut count,
			)
		})?;
		let length: usize = usize::try_from(count).map_err(|_| Error::InvalidCount(count))?;
		if length > max_frames {
			return Err(Error::InvalidCount(count));
		}
		frames.truncate(length);
		Ok(frames)
	}

	/// Suspends one platform or virtual thread (JVMTI 1.2; `can_suspend`).
	///
	/// Null suspends the current thread and blocks until another thread resumes it.
	/// Suspension is not nested; suspending an already suspended thread is an error.
	/// No JNI reference ownership is transferred and no automatic resumption occurs.
	///
	/// # Errors
	/// Returns native thread/capability errors or a missing function.
	///
	/// # Safety
	/// The phase, thread, and callback rules of [`Self::stack_trace`] apply. Arrange
	/// resumption on every path, coordinate with other agent users, and avoid
	/// suspending a thread holding locks needed by the suspending/resuming thread.
	pub unsafe fn suspend_thread(&self, thread: sys::jthread) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jthread) -> sys::jvmtiError = self
			.table()
			.SuspendThread
			.ok_or(Error::MissingFunction("SuspendThread"))?;
		// SAFETY: Caller supplies a valid thread, context, and resumption strategy.
		check(unsafe { function(self.as_raw(), thread) })
	}

	/// Resumes a suspended platform or virtual thread (JVMTI 1.2; `can_suspend`).
	///
	/// # Errors
	/// Returns native errors including `THREAD_NOT_SUSPENDED`, or a missing function.
	///
	/// # Safety
	/// The phase, thread, callback, coordination, and ownership rules of
	/// [`Self::suspend_thread`] apply. This invalidates assumptions of suspended frames.
	pub unsafe fn resume_thread(&self, thread: sys::jthread) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jthread) -> sys::jvmtiError = self
			.table()
			.ResumeThread
			.ok_or(Error::MissingFunction("ResumeThread"))?;
		// SAFETY: Caller supplies a valid thread and coordinates resumption.
		check(unsafe { function(self.as_raw(), thread) })
	}

	/// Suspends all virtual threads except the supplied list (JVMTI 21).
	///
	/// Requires `can_suspend` and `can_support_virtual_threads`. Already suspended
	/// threads stay suspended. The VM does not retain the borrowed reference array.
	///
	/// # Errors
	/// Returns unsupported-version, missing-function, size, or native errors.
	///
	/// # Safety
	/// The live-phase, callback, thread, deadlock, and resumption rules of
	/// [`Self::suspend_thread`] apply. Every exception must be a valid thread
	/// reference. Exclude the calling thread if it is virtual. Coordinate globally:
	/// a later bulk resume can also resume threads suspended by other agent users.
	pub unsafe fn suspend_all_virtual_threads(&self, except: &[sys::jthread]) -> Result<()> {
		self.ensure_version(JvmtiVersion::V21)?;
		let count: sys::jint = super::management::count(except.len())?;
		// SAFETY: The version check precedes borrowing the JVMTI 21 table.
		let table: &sys::jvmtiInterface_1__21 = unsafe { &*(*self.as_raw()).cast::<sys::jvmtiInterface_1__21>() };
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jint, *const sys::jthread) -> sys::jvmtiError =
			table
				.SuspendAllVirtualThreads
				.ok_or(Error::MissingFunction("SuspendAllVirtualThreads"))?;
		// SAFETY: Caller supplies valid references and context; the exception list stays readable.
		check(unsafe { function(self.as_raw(), count, except.as_ptr()) })
	}

	/// Resumes all virtual threads except the supplied list (JVMTI 21).
	///
	/// Requires `can_suspend` and `can_support_virtual_threads`. Threads not
	/// suspended remain unchanged; no input ownership is transferred.
	///
	/// # Errors
	/// Returns unsupported-version, missing-function, size, or native errors.
	///
	/// # Safety
	/// The phase, callback, thread, reference, and global coordination requirements
	/// of [`Self::suspend_all_virtual_threads`] apply.
	pub unsafe fn resume_all_virtual_threads(&self, except: &[sys::jthread]) -> Result<()> {
		self.ensure_version(JvmtiVersion::V21)?;
		let count: sys::jint = super::management::count(except.len())?;
		// SAFETY: The version check precedes borrowing the JVMTI 21 table.
		let table: &sys::jvmtiInterface_1__21 = unsafe { &*(*self.as_raw()).cast::<sys::jvmtiInterface_1__21>() };
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jint, *const sys::jthread) -> sys::jvmtiError =
			table
				.ResumeAllVirtualThreads
				.ok_or(Error::MissingFunction("ResumeAllVirtualThreads"))?;
		// SAFETY: Caller supplies valid references and context; the exception list stays readable.
		check(unsafe { function(self.as_raw(), count, except.as_ptr()) })
	}
}
