use crate::{
	Env,
	errors::{Error, Result},
	jni_str,
	objects::{IntoAuto as _, JObject, JThrowable},
	refs::Auto,
};

crate::bind_java_type! {
	pub JIterator => "java.util.Iterator",
	methods {
		/// Returns true if the iteration has more elements.
		fn has_next() -> bool,

		priv fn _next() -> JObject,

		/// Removes the current element from the iteration (if supported by the iterator)
		///
		/// This can only be called once after [Self::next] is called.
		///
		/// # Throws
		///
		/// - `UnsupportedOperationException` if the operation is not supported.
		/// - `IllegalStateException` if the iterator is in an invalid state (i.e [Self::next] has not been called).
		fn remove()
	}
}

impl JIterator<'_> {
	/// Returns the next element in the iteration, if it exists.
	///
	/// Returns `Some(element)` if the iteration has more elements, or `None` if
	/// it has reached the end.
	///
	/// This is like [`std::iter::Iterator::next`], but requires a parameter of
	/// type `&mut Env` in order to call into Java.
	///
	/// Checks [`Self::has_next`] before calling Java's `next`, since some Android
	/// iterators return invalid entries instead of throwing on exhaustion. Each
	/// successful step therefore calls both `hasNext` and `next` through JNI.
	/// A `NoSuchElementException` thrown by `next` is still caught and cleared
	/// before returning `None`. Exceptions from `hasNext` and other exceptions
	/// from `next` remain pending and are propagated as [`Error::JavaException`].
	///
	/// ## Beware of creating excessive local references in the current JNI stack frame
	///
	/// This method creates a new local reference. To prevent excessive memory
	/// usage or overflow errors (when called repeatedly in a loop), the local
	/// reference should be deleted using [`Env::delete_local_ref`] or wrapped
	/// with [`crate::objects::IntoAuto::auto`] before the next loop iteration.
	/// Alternatively, if the collection is known to have a small, predictable
	/// size, the loop could be wrapped in [`Env::with_local_frame`] to delete
	/// all of the local references at once.
	///
	/// # Errors
	///
	/// Propagates JNI failures, all exceptions from `hasNext`, and exceptions from `next`
	/// other than `NoSuchElementException`.
	/// An exception already pending on entry is propagated without advancing the iterator.
	pub fn next<'env_local>(&self, env: &mut Env<'env_local>) -> Result<Option<JObject<'env_local>>> {
		// A pre-existing exception must not be mistaken for iterator exhaustion.
		if env.exception_check() {
			return Err(Error::JavaException);
		}

		if !self.has_next(env)? {
			return Ok(None);
		}

		match self._next(env) {
			Ok(v) => Ok(Some(v)),
			Err(Error::JavaException) => {
				let exception: Auto<'_, JThrowable<'_>> = env.exception_occurred().ok_or(Error::JavaException)?.auto();
				// JNI type checks require clearing the pending exception first.
				env.exception_clear();
				if matches!(
					env.is_instance_of(&exception, jni_str!("java/util/NoSuchElementException")),
					Ok(true)
				) {
					Ok(None)
				} else {
					// Preserve the original failure even if inspecting its type failed.
					env.exception_clear();
					env.throw(&*exception)?;
					Err(Error::JavaException)
				}
			}
			Err(e) => Err(e),
		}
	}
}
