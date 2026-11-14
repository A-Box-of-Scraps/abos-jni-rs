use crate::sys;

use super::{
	DEFAULT_LOCAL_FRAME_CAPACITY, Desc, Env, Error, JClass, JNIStr, JThrowable, LoaderContext, Reference, Result,
};
use std::fmt::Write as _;

impl<'local> Env<'local> {
	/// Raise an exception from an existing object. This will continue being thrown in java unless
	/// `exception_clear` is called.
	///
	/// Returns [`Error::JavaException`] after throwing the exception.
	///
	/// The '?' operator should typically used to ensure that the new pending exception is reported
	/// as an error to calling Rust code, allowing the stack to unwind until something decides to
	/// catch the exception (or it propagates back to the JVM).
	///
	/// *Note:* that once there is a pending exception then most JNI calls (that are not exception
	/// safe) will return [`Error::JavaException`] until the exception is cleared.
	///
	/// # Examples
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// env.throw((jni_str!("java/lang/Exception"), jni_str!("something bad happened")))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Defaulting to "java/lang/Exception":
	///
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// env.throw(jni_str!("something bad happened"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns `Error::JavaException` after throwing, or an error if resolving or throwing the exception fails.
	pub fn throw<'other_local, E>(&mut self, obj: E) -> Result<()>
	where
		E: Desc<'local, JThrowable<'other_local>>,
	{
		let throwable: <E as Desc<'_, JThrowable<'_>>>::Output = obj.lookup(self)?;

		// Safety:
		// Throw is 1.1 API that must be valid
		//
		// We are careful to ensure that we don't drop the reference
		// to `throwable` after converting to a raw pointer.
		let res: i32 = unsafe { jni_call_no_post_check_ex!(self, v1_1, Throw, throwable.as_ref().as_raw())? };

		// Ensure that `throwable` isn't dropped before the JNI call returns.
		drop(throwable);

		if res == 0 {
			Err(Error::JavaException)
		} else {
			Err(Error::ThrowFailed(res))
		}
	}

	pub(super) fn throw_new_optional(&self, class: &JClass, msg: Option<&JNIStr>) -> Result<()> {
		let class: &JClass<'_> = null_check!(class, "throw_new class")?;
		let throwable_class = JThrowable::lookup_class(self, &LoaderContext::None)?;
		let throwable_class: &JClass = &throwable_class;

		if !self.is_assignable_from_class(class.as_ref(), throwable_class)? {
			return Err(Error::WrongObjectType);
		}
		let msg: Option<&JNIStr> = msg.as_ref().map(std::convert::AsRef::as_ref);

		// Safety:
		// ThrowNew is 1.1 API that must be valid
		//
		// We are careful to ensure that we don't drop the reference
		// to `class` or `msg` after converting to raw pointers.
		let res: i32 = unsafe {
			jni_call_no_post_check_ex!(
				self,
				v1_1,
				ThrowNew,
				class.as_raw(),
				msg.map_or(std::ptr::null(), JNIStr::as_ptr)
			)?
		};

		if res == 0 {
			Err(Error::JavaException)
		} else {
			Err(Error::ThrowFailed(res))
		}
	}

	/// Create and throw a new exception from a class descriptor and an error message.
	///
	/// The '?' operator should typically used to ensure that the new pending exception is reported
	/// as an error to calling Rust code, allowing the stack to unwind until something decides to
	/// catch the exception (or it propagates back to the JVM).
	///
	/// *Note:* that once there is a pending exception then most JNI calls (that are not exception
	/// safe) will return [`Error::JavaException`] until the exception is cleared.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// env.throw_new(jni_str!("java/lang/Exception"), jni_str!("something bad happened"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Alternatively, see [`Env::throw_new_void`] if you want to construct an exception with no
	/// message argument.
	///
	/// Use [`Self::throw_new_from_class`] with an already-resolved class and a shared environment.
	///
	/// # Errors
	///
	/// Returns `Error::JavaException` after throwing, or an error if resolving or constructing the exception fails.
	pub fn throw_new<'other_local, S, T>(&mut self, class: T, msg: S) -> Result<()>
	where
		S: AsRef<JNIStr>,
		T: Desc<'local, JClass<'other_local>>,
	{
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let msg: &JNIStr = msg.as_ref();
		self.throw_new_from_class(class.as_ref(), msg)
	}

	/// Creates and throws an exception from an already-resolved class using a shared environment.
	///
	/// See [`Self::throw_new`] for exception behavior.
	///
	/// # Errors
	///
	/// Returns `Error::JavaException` after throwing, or an error if the class is null,
	/// is not a throwable class, or constructing the exception fails.
	pub fn throw_new_from_class<S>(&self, class: &JClass<'_>, msg: S) -> Result<()>
	where
		S: AsRef<JNIStr>,
	{
		self.throw_new_optional(class, Some(msg.as_ref()))
	}

	/// Create and throw a new exception from a class descriptor and no error message.
	///
	/// The '?' operator should typically used to ensure that the new pending exception is reported
	/// as an error to calling Rust code, allowing the stack to unwind until something decides to
	/// catch the exception (or it propagates back to the JVM).
	///
	/// *Note:* that once there is a pending exception then most JNI calls (that are not exception
	/// safe) will return [`Error::JavaException`] until the exception is cleared.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// env.throw_new_void(jni_str!("java/lang/Exception"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// This will expect to find a constructor for the given `class` that takes no arguments.
	///
	/// Alternatively, see [`Env::throw_new`] if you want to construct an exception with a message.
	///
	/// # Errors
	///
	/// Returns `Error::JavaException` after throwing, or an error if resolving or constructing the exception fails.
	pub fn throw_new_void<'other_local, T>(&mut self, class: T) -> Result<()>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		self.throw_new_optional(class.as_ref(), None)
	}

	/// Returns true if an exception is currently in the process of being thrown.
	///
	/// This doesn't need to create any local references
	#[inline]
	#[must_use]
	pub fn exception_check(&self) -> bool {
		// Safety: ExceptionCheck is 1.2 API, which we check for in `from_raw()`
		unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_2, ExceptionCheck) != sys::JNI_FALSE }
	}

	/// Check whether or not an exception is currently in the process of being
	/// thrown.
	///
	/// An exception is in this state from the time it gets thrown and
	/// not caught in a java function until `exception_clear` is called.
	pub fn exception_occurred(&mut self) -> Option<JThrowable<'local>> {
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		// SAFETY: ExceptionOccurred is valid with a pending exception and returns a local throwable reference.
		let throwable: crate::sys::jobject =
			unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_1, ExceptionOccurred) };
		if throwable.is_null() {
			None
		} else {
			// SAFETY: The non-null pointer came from ExceptionOccurred in the current local frame.
			Some(unsafe { JThrowable::from_raw(self, throwable) })
		}
	}

	/// Print exception information to the console.
	pub fn exception_describe(&self) {
		// Safety: ExceptionDescribe is 1.1 API that must be valid
		unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_1, ExceptionDescribe) };
	}

	/// Clear an exception in the process of being thrown. If this is never
	/// called, the exception will continue being thrown when control is
	/// returned to java.
	pub fn exception_clear(&self) {
		// Safety: ExceptionClear is 1.1 API that must be valid
		unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_1, ExceptionClear) };
	}

	/// Catch a pending Java exception and convert it into a Rust
	/// [`Error::CaughtJavaException`].
	///
	/// If a Java exception is pending, it will be cleared and converted into a
	/// Rust [`Error::CaughtJavaException`].
	///
	/// # Errors
	///
	/// Returns `Error::CaughtJavaException` for a pending exception, or an error encountered while inspecting it.
	///
	/// # Panics
	///
	/// Panics if JNI reports a pending exception but returns a null throwable.
	pub fn exception_catch(&self) -> Result<()> {
		pub(super) fn try_get_stack_trace(
			env: &mut abos_jni::Env<'_>,
			throwable: &abos_jni::objects::JThrowable,
		) -> abos_jni::errors::Result<String> {
			let stack_trace: crate::objects::JObjectArray<'_, crate::objects::JStackTraceElement<'_>> =
				throwable.get_stack_trace(env)?;
			let len: usize = stack_trace.len(env)?;
			let mut trace = String::new();
			for i in 0..len {
				let element: crate::objects::JStackTraceElement<'_> = stack_trace.get_element(env, i)?;
				let element_jstr: crate::objects::JString<'_> = element.try_to_string(env)?;
				writeln!(trace, "{i}: {element_jstr}").expect("writing to String cannot fail");
			}
			Ok(trace)
		}

		if !self.exception_check() {
			return Ok(());
		}
		let result: std::result::Result<crate::errors::Error, crate::errors::Error> = self.with_local_frame(
			DEFAULT_LOCAL_FRAME_CAPACITY,
			|env| -> abos_jni::errors::Result<abos_jni::errors::Error> {
				let Some(e) = env.exception_occurred() else {
					// should only be called after receiving a JavaException Result
					unreachable!("ExceptionOccurred returned None after ExceptionCheck returned true");
				};
				env.exception_clear();

				assert!(!e.is_null(), "ExceptionOccurred returned a null throwable");

				let e: crate::refs::Global<JThrowable<'_>> = env.new_global_ref(e)?;
				// besides null pointer checks, there are no documented errors or exceptions
				// for GetObjectClass or getName, so this should be infallible
				let name: String = env.get_object_class(&e)?.get_name(env)?.to_string();
				let message: crate::objects::JString<'_> = e.get_message(env)?;
				let msg: String = if message.is_null() {
					String::new()
				} else {
					message.try_to_string(env)?
				};

				let stack: String = match try_get_stack_trace(env, &e) {
					Ok(stack_trace) => stack_trace,
					Err(err) => format!("none: {err:?}"),
				};

				Ok(abos_jni::errors::Error::CaughtJavaException {
					exception: e,
					name,
					msg,
					stack,
				})
			},
		);

		match result {
			Ok(exception) => Err(exception),
			Err(err) => Err(abos_jni::errors::Error::CaughtJavaException {
				exception: crate::refs::Global::default(),
				name: "UNKNOWN".to_string(),
				msg: format!("Failed to query JThrowable: {err:?})"),
				stack: "none".to_string(),
			}),
		}
	}

	/// Abort the JVM with an error message.
	///
	/// This method is guaranteed not to panic, and only calls `ExceptionClear`
	/// prior to calling [`FatalError`]. The `jni-rs` wrapper function does not
	/// perform any heap allocations (although `FatalError` might perform heap
	/// allocations of its own).
	///
	/// In exchange for these strong guarantees, this method requires an error
	/// message to already be suitably encoded, as described in the
	/// documentation for the [`JNIStr`] type.
	///
	/// The simplest way to use this is to convert a string to a [`JNIStr`] via
	/// [`abos_jni::jni_str!`] like so:
	///
	/// ```no_run
	/// # use abos_jni::{jni_str, Env, strings::JNIStr};
	/// # let env: Env = unimplemented!();
	/// env.fatal_error(jni_str!("Game over!"))
	/// ```
	///
	/// [`FatalError`]:
	///     https://docs.oracle.com/en/java/javase/21/docs/specs/jni/functions.html#fatalerror
	/// [`ExceptionClear`]:
	///     https://docs.oracle.com/en/java/javase/21/docs/specs/jni/functions.html#exceptionclear
	/// [Modified UTF-8]:
	///     https://docs.oracle.com/en/java/javase/21/docs/specs/jni/types.html#modified-utf-8-strings
	pub fn fatal_error(&self, msg: &JNIStr) -> ! {
		// Since FatalError is not considered to safe to call with a pending exception
		// we have to first clear any pending exception.
		self.exception_clear();

		// Safety: FatalError is 1.1 API that must be valid
		//
		// Very little is specified about the implementation of FatalError but we still
		// currently consider this "safe", similar to how `abort()` is considered safe.
		// It won't give the application an opportunity to clean or save state but the
		// process will be terminated.
		//
		// Although FatalError is not exception safe, we call it as if it is because
		// we have guaranteed that there are no pending exceptions to check for.
		unsafe { ex_safe_jni_call_no_post_check_ex!(self, v1_1, FatalError, msg.as_ptr()) }
	}
}
