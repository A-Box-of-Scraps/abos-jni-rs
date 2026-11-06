#[cfg(doc)]
use crate::refs::Global;

use super::{
	AssertUnwindSafe, AttachGuard, Env, Error, JObject, JniError, Reference, Result, TryInto, catch_unwind,
	jni_error_code_to_result, resume_unwind,
};

impl<'local> Env<'local> {
	/// Creates a new local reference frame, in which at least a given number
	/// of local references can be created.
	///
	/// Returns `Err` on failure, with a pending `OutOfMemoryError`.
	///
	/// Prefer to use
	/// [`with_local_frame`](struct.Env.html#method.with_local_frame)
	/// instead of direct `push_local_frame`/`pop_local_frame` calls.
	///
	/// # Safety
	///
	/// The caller must guarantee that there will be a corresponding call to
	/// `pop_local_frame`
	///
	/// The caller must ensure that a new `AttachGuard` is created before
	/// creating a new local frame and the local frame may only access
	/// a `Env` that is borrowed from this new guard (so that local
	/// references will be tied to lifetime of the new guard)
	pub(super) unsafe fn push_local_frame(&self, capacity: i32) -> Result<()> {
		// Safety:
		// This method is safe to call in case of pending exceptions (see chapter 2 of the spec)
		// We check for JNI > 1.2 in `from_raw`
		let res = unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_2, PushLocalFrame, capacity) };
		jni_error_code_to_result(res)
	}

	/// Pops off the current local reference frame, frees all the local
	/// references allocated on the current stack frame, except the `result`,
	/// which is returned from this function and remains valid.
	///
	/// The resulting `JObject` will be `NULL` iff `result` is `NULL`.
	///
	/// This method allows direct control of local frames, but it can cause
	/// undefined behavior and is therefore unsafe. Prefer
	/// [`Env::with_local_frame`] instead.
	///
	/// # Safety
	///
	/// Any local references created after the most recent call to
	/// [`Env::push_local_frame`] (or the underlying JNI function) must not
	/// be used after calling this method.
	///
	/// The `AttachGuard` created before calling `push_local_frame` must be
	/// dropped after calling `pop_local_frame`.
	pub(super) unsafe fn pop_local_frame<'frame_local, T: Reference>(
		&self,
		result: T::Kind<'frame_local>,
	) -> T::Kind<'local> {
		let result: JObject<'frame_local> = result.into();
		// Safety:
		// This method is safe to call in case of pending exceptions (see chapter 2 of the spec)
		// We check for JNI > 1.2 in `from_raw`
		unsafe {
			let raw = ex_safe_jni_call_no_post_check_ex!(self, v1_2, PopLocalFrame, result.into_raw());
			T::kind_from_raw(raw)
		}
	}

	/// Run a closure with a mutable [`Env`] associated with a new local reference frame.
	///
	/// `capacity` is the minimum number of local references that can be created in the new frame.
	/// it can be seen as a hint to the JVM to pre-allocate space for that many local references
	/// but if more are created then the JVM will transparently allocate more space as needed.
	///
	/// If a frame can't be allocated with the requested capacity for local references, returns
	/// `Err` with a pending `OutOfMemoryError`.
	///
	/// Once this method returns, all references allocated in the frame are freed.
	///
	/// If you need to return a reference from the frame to the caller then you should either
	/// use [`Self::with_local_frame_returning_local`] or else create a [`Global`] with
	/// [`Self::new_global_ref`].
	///
	/// # Runtime Top Frame Checks
	///
	/// See top-level [Env] documentation for rules on limiting yourself to one
	/// [Env] reference per-scope to avoid exposing code to runtime checks for
	/// the top JNI frame (that can panic).
	///
	/// # Errors
	///
	/// Returns an error if capacity does not fit JNI, a local frame cannot be allocated, or the callback returns an error.
	pub fn with_local_frame<F, T, E>(&self, capacity: usize, f: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		let capacity: abos_jni_sys::jint = capacity
			.try_into()
			.map_err(|_| Error::JniCall(JniError::InvalidArguments))?;

		unsafe {
			// Safety: by creating a new AttachGuard we ensure that the attach guard level
			// will be incremented in sync with the creation of a new JNI stack frame
			let mut guard = AttachGuard::from_unowned(self.get_raw());
			let env = guard.borrow_env_mut();
			self.push_local_frame(capacity)?;
			let ret = catch_unwind(AssertUnwindSafe(|| f(env)));
			self.pop_local_frame::<JObject>(JObject::null());
			drop(guard);

			match ret {
				Ok(ret) => ret,
				Err(payload) => {
					resume_unwind(payload);
				}
			}
		}
	}

	/// Run a closure with a mutable [`Env`] associated with a new local reference frame, and return
	/// one local reference to the caller.
	///
	/// `capacity` is the minimum number of local references that can be created in the new frame.
	/// it can be seen as a hint to the JVM to pre-allocate space for that many local references but
	/// if more are created then the JVM will transparently allocate more space as needed.
	///
	/// If a frame can't be allocated with the requested capacity for local references, returns
	/// `Err` with a pending `OutOfMemoryError`.
	///
	/// Once this method returns, all references allocated in the frame are freed, except the one
	/// that the function returns, which remains valid.
	///
	/// Since the low-level JNI interface has support for passing back a single local reference from
	/// a local frame as special-case optimization, this alternative to `with_local_frame` exposes
	/// that capability to return a local reference without needing to create a temporary
	/// [`Global`].
	///
	/// # Errors
	///
	/// Returns an error if capacity does not fit JNI, a local frame cannot be allocated, or the callback returns an error.
	pub fn with_local_frame_returning_local<F, T, E>(
		&mut self,
		capacity: usize,
		f: F,
	) -> std::result::Result<T::Kind<'local>, E>
	where
		F: for<'new_local> FnOnce(&mut Env<'new_local>) -> std::result::Result<T::Kind<'new_local>, E>,
		T: Reference,
		E: From<Error>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();

		let capacity: abos_jni_sys::jint = capacity
			.try_into()
			.map_err(|_| Error::JniCall(JniError::InvalidArguments))?;

		// SAFETY: The nested guard tracks the pushed JNI frame, which is popped before the guard drops on every callback outcome.
		unsafe {
			// Inner scope to ensure drop order: `Result` -> `env` -> `guard`, before we potentially call `resume_unwind`.
			let panic_payload = {
				// Safety: by creating a new AttachGuard we ensure that the attach guard level
				// will be incremented in sync with the creation of a new JNI stack frame
				let mut guard = AttachGuard::from_unowned(self.get_raw());
				let env = guard.borrow_env_mut();

				self.push_local_frame(capacity)?;

				match catch_unwind(AssertUnwindSafe(|| f(env))) {
					Ok(Ok(obj)) => {
						let obj = self.pop_local_frame::<T>(obj);
						return Ok(obj);
					}
					Ok(Err(err)) => {
						self.pop_local_frame::<T>(T::null());
						return Err(err);
					}
					Err(payload) => {
						self.pop_local_frame::<T>(T::null());
						payload
					}
				}
			};

			resume_unwind(panic_payload);
		}
	}

	/// Runs a closure with a mutable [`Env`] associated with the top local reference frame.
	///
	/// Unlike [`Self::with_local_frame()`], this API does not push a new JNI stack frame and so new
	/// local references created in the closure will be associated with the existing local reference
	/// frame at the top of the stack.
	///
	/// Most of the time this API should probably be avoided (see [`Self::with_local_frame`]).
	///
	/// Only use if:
	/// - You're sure your code won't leak local references into the current stack frame.
	/// - OR, you're sure that the leaked references are acceptable because you know when the top
	///   frame will unwind and release those references.
	///
	/// This will have a slightly lower overhead than [`Self::with_local_frame()`] (since it doesn't
	/// need to push/pop a JNI stack frame), but the trade off is that you may leak local references
	/// into the top stack frame.
	///
	/// Keep in mind that deleting local references individually is likely to have a higher cost
	/// than pushing/popping a JNI stack frame, so you should probably only use this API if you're
	/// OK with leaking a small number of local references into the top frame and waiting for it to
	/// unwind.
	///
	/// # Runtime Top Frame Checks
	///
	/// See top-level [Env] documentation for rules on limiting yourself to one [Env] reference
	/// per-scope to avoid exposing code to runtime checks for the top JNI frame (that can panic).
	///
	/// # Errors
	///
	/// Propagates the error returned by the callback.
	pub fn with_top_local_frame<F, T, E>(&self, f: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		// SAFETY: The nested guard borrows the current attached thread's environment and drops after the callback.
		unsafe {
			let mut guard = AttachGuard::from_unowned(self.get_raw());
			f(guard.borrow_env_mut())
		}
	}
}
