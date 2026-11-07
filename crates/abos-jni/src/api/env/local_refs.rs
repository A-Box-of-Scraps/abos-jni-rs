#[cfg(doc)]
use crate::refs::{Global, IntoAuto};

use super::{Auto, Cast, Env, Error, JObject, JniError, Reference, Result, jobject};

impl<'local> Env<'local> {
	/// Create a new local reference to an object.
	///
	/// Specifically, this calls the JNI function [`NewLocalRef`], which creates a reference in the
	/// current local reference frame, regardless of whether the original reference belongs to the
	/// same local reference frame, a different one, or is a [global reference][Global]. In Rust
	/// terms, this method accepts a JNI reference with any valid lifetime and produces a clone of
	/// that reference with the lifetime of this `Env`. The returned reference can outlive the
	/// original.
	///
	/// This method is useful when you have a strong global reference and you can't prevent it from
	/// being dropped before you're finished with it. In that case, you can use this method to
	/// create a new local reference that's guaranteed to remain valid for the duration of the
	/// current local reference frame, regardless of what later happens to the original global
	/// reference.
	///
	/// # Lifetimes
	///
	/// `'local` is the lifetime of the local reference frame that this `Env` belongs to. This
	/// method creates a new local reference in that frame, with lifetime `'local`.
	///
	/// The original reference can belong to any live local frame or be a global or weak global
	/// reference. Its lifetime need not match `'local`. If a weak referent has been collected,
	/// this method returns [`Error::ObjectFreed`] instead of a new local reference.
	///
	/// # Example
	///
	/// In the following example, the `ExampleError::extract_throwable` method uses
	/// `Env::new_local_ref` to create a new local reference that outlives the original global
	/// reference:
	///
	/// ```no_run
	/// # use abos_jni::{jni_sig, jni_str, Env, objects::*, strings::*};
	/// # use std::fmt::Display;
	/// #
	/// # type SomeOtherErrorType = Box<dyn Display>;
	/// #
	/// /// An error that may be caused by either a Java exception or something going wrong in Rust
	/// /// code.
	/// enum ExampleError {
	///     /// This variant represents a Java exception.
	///     ///
	///     /// The enclosed `Global` points to a Java object of class `java.lang.Throwable`
	///     /// (or one of its many subclasses).
	///     Exception(Global<JObject<'static>>),
	///
	///     /// This variant represents an error in Rust code, not a Java exception.
	///     Other(SomeOtherErrorType),
	/// }
	///
	/// impl ExampleError {
	///     /// Consumes this `ExampleError` and produces a `JThrowable`, suitable for throwing
	///     /// back to Java code.
	///     ///
	///     /// If this is an `ExampleError::Exception`, then this extracts the enclosed Java
	///     /// exception object. Otherwise, a new exception object is created to represent this
	///     /// error.
	///     fn extract_throwable<'local>(self, env: &mut Env<'local>) -> abos_jni::errors::Result<JThrowable<'local>> {
	///         let throwable: JObject = match self {
	///             ExampleError::Exception(exception) => {
	///                 // The error was caused by a Java exception.
	///
	///                 // Here, `exception` is a `Global` pointing to a Java `Throwable`. It
	///                 // will be dropped at the end of this `match` arm. We'll use
	///                 // `new_local_ref` to create a local reference that will outlive the
	///                 // `Global`.
	///
	///                 env.new_local_ref(&exception)?
	///             }
	///
	///             ExampleError::Other(error) => {
	///                 // The error was caused by something that happened in Rust code. Create a
	///                 // new `java.lang.Error` to represent it.
	///
	///                 let error_string = JString::from_str(env, error.to_string())?;
	///
	///                 env.new_object(
	///                     jni_str!("java/lang/Error"),
	///                     jni_sig!("(Ljava/lang/String;)V"),
	///                     &[
	///                         (&error_string).into(),
	///                     ],
	///                 )?
	///             }
	///         };
	///         let throwable = env.cast_local::<JThrowable>(throwable)?;
	///         Ok(throwable)
	///     }
	/// }
	/// ```
	///
	/// [`NewLocalRef`]: https://docs.oracle.com/en/java/javase/11/docs/specs/jni/functions.html#newlocalref
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate a local reference or a weak source reference has already been collected.
	pub fn new_local_ref<O>(&mut self, obj: O) -> Result<O::Kind<'local>>
	where
		O: Reference,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();

		// By checking for `null` before calling `NewLocalRef` we can recognise
		// that a `null` returned from `NewLocalRef` is from being out of memory.
		if obj.is_null() {
			return Ok(O::null());
		}

		// Safety:
		// - the minimum supported JNI version is 1.4
		// - we can assume that `obj.raw()` is a valid reference, or null
		// - we know there's no other wrapper for the reference passed to from_local_raw
		//   since we have just created it.
		let local: O::Kind<'_> =
			unsafe { O::kind_from_raw(jni_call_no_post_check_ex!(self, v1_2, NewLocalRef, obj.as_raw())?) };

		// Per JNI spec, `NewLocalRef` will return a null pointer if the object was GC'd
		// (which could happen if `obj` is a `Weak`):
		//
		//  > it is recommended that a (strong) local or global reference to the
		//  > underlying object be acquired using one of the JNI functions
		//  > NewLocalRef or NewGlobalRef. These functions will return NULL if
		//  > the object has been freed.
		//
		if local.is_null() {
			// In this case it's ambiguous whether there has been an out-of-memory error or
			// the object has been garbage collected and so we now _explicitly_ check
			// whether the object has been garbage collected.
			if self.is_same_object(obj, JObject::null())? {
				Err(Error::ObjectFreed)
			} else {
				Err(Error::JniCall(JniError::NoMemory))
			}
		} else {
			Ok(local)
		}
	}

	/// Creates a new local reference and casts it to a different type.
	///
	/// This is a convenience method that combines [`Self::cast_local`] and [`Self::new_local_ref`].
	///
	/// This performs a runtime type check using `IsInstanceOf` and then creates a new local
	/// reference.
	///
	/// `obj` can be a local reference or a global reference.
	///
	/// For upcasting (converting to a more general type), consider using the `From` trait
	/// implementations instead, which don't require runtime checks.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let local_obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let local_string = env.new_cast_local_ref::<JString>(&local_obj)?;
	/// // local_string is now a JString<'local> in the current frame
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of the target type.
	/// Returns [`Error::ClassNotFound`] if the target class cannot be found.
	pub fn new_cast_local_ref<'any_local, To>(
		&mut self,
		obj: impl Reference + AsRef<JObject<'any_local>>,
	) -> Result<To::Kind<'local>>
	where
		To: Reference,
	{
		if obj.is_null() {
			return Ok(To::null());
		}

		if self.is_instance_of_cast_type::<To>(obj.as_ref())? {
			let new: JObject<'_> = self.new_local_ref(obj.as_ref())?;
			// Safety:
			// - we have just checked that `new` is an instance of `To`
			// - as it's a new reference, it's assigned the `'local` Env lifetime
			unsafe { Ok(To::kind_from_raw::<'local>(new.into_raw())) }
		} else {
			Err(Error::WrongObjectType)
		}
	}

	/// Attempts to cast an owned local reference to a different type.
	///
	/// This performs a runtime type check using `IsInstanceOf` and consumes the input reference.
	/// For upcasting (converting to a more general type), consider using the `From` trait
	/// implementations instead, which don't require runtime checks.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let string: JString = env.cast_local::<JString>(obj)?;
	///
	/// // For upcasting, From trait is more efficient:
	/// let obj_again: JObject = string.into(); // No runtime check needed
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of the target type.
	/// Returns [`Error::ClassNotFound`] if the target class cannot be found.
	pub fn cast_local<'any_local, To>(
		&self,
		obj: impl Reference + Into<JObject<'any_local>> + AsRef<JObject<'any_local>>,
	) -> Result<To::Kind<'any_local>>
	where
		To: Reference,
	{
		if obj.is_null() {
			return Ok(To::null::<'any_local>());
		}

		if self.is_instance_of_cast_type::<To>(obj.as_ref())? {
			let obj: JObject = obj.into();
			// Safety:
			// - we have just checked that `obj` is an instance of `T`
			// - it is associated with the same lifetime that it was created with
			unsafe { Ok(To::kind_from_raw::<'any_local>(obj.into_raw())) }
		} else {
			Err(Error::WrongObjectType)
		}
	}

	/// Attempts to cast a reference (local or global) to a different type
	/// without consuming it.
	///
	/// This method borrows the input reference and returns a wrapper that
	/// derefs to the target type. The original reference remains valid and can
	/// be used after the cast operation.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let string_ref = env.as_cast::<JString>(&obj)?;
	/// // obj is still valid here
	/// let empty_string_contents = string_ref.to_string();
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of
	/// the target type. Returns [`Error::ClassNotFound`] if the target class
	/// cannot be found.
	pub fn as_cast<'from, 'any, To>(
		&self,
		obj: &'from (impl Reference + AsRef<JObject<'any>>),
	) -> Result<Cast<'from, 'any, To>>
	where
		To: Reference,
		'any: 'from,
	{
		Cast::new(self, obj)
	}

	/// Cast a reference (local or global) to a different type without consuming
	/// it and without any runtime checks.
	///
	/// This method borrows the input reference and returns a wrapper that
	/// derefs to the target type. The original reference remains valid and can
	/// be used after the cast operation.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let string_ref = unsafe { env.as_cast_unchecked::<JString>(&obj) };
	/// // obj is still valid here
	/// let empty_string_contents = string_ref.to_string();
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Safety
	///
	/// The caller must ensure that `obj` is a valid instance of `To`, or
	/// `null`.
	pub unsafe fn as_cast_unchecked<'from, 'any, To>(
		&self,
		obj: &'from (impl Reference + AsRef<JObject<'any>>),
	) -> Cast<'from, 'any, To>
	where
		To: Reference,
		'any: 'from,
	{
		// Safety: the caller must ensure that `obj` is a valid instance of `To`, or `null`.
		unsafe { Cast::new_unchecked(obj) }
	}

	/// Attempts to cast a raw [`jobject`] reference without taking ownership.
	///
	/// This method borrows the input reference and returns a wrapper that
	/// derefs to the target type. The original reference remains valid and can
	/// continue to be used.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env, objects::*, sys::jobject};
	/// #
	/// # fn example(env: &mut Env, raw_global: jobject) -> Result<()> {
	/// // SAFETY: we know that raw_global is a valid java.lang.String reference
	/// let string_ref = unsafe { env.as_cast_raw::<Global<JString>>(&raw_global)? };
	/// let string_contents = string_ref.to_string();
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of
	/// the target type. Returns [`Error::ClassNotFound`] if the target class
	/// cannot be found.
	///
	/// # Safety
	///
	/// The caller must ensure that `from` is a valid reference (local or
	/// global) - which may be `null`.
	///
	/// The caller must ensure the `from` reference will not be deleted while
	/// the `Cast` exists.
	///
	/// Note: even though this API is `unsafe`, it will still do a runtime check
	/// that `from` is a valid instance of `To`, so you are not required to know
	/// this.
	///
	/// Note: this API is agnostic about whether the reference is local or
	/// global because the returned [`Cast`] wrapper doesn't give ownership over
	/// the reference and so you can't accidentally attempt to delete it using
	/// the wrong JNI API.
	pub unsafe fn as_cast_raw<'from, To>(&self, from: &'from jobject) -> Result<Cast<'from, 'from, To>>
	where
		To: Reference,
	{
		// Safety: the rules in the doc comment cover the `from_raw` safety requirements
		unsafe { Cast::from_raw(self, from) }
	}

	/// Creates a new auto-deleted local reference.
	///
	/// See also [`with_local_frame`](struct.Env.html#method.with_local_frame) method that
	/// can be more convenient when you create a _bounded_ number of local references
	/// but cannot rely on automatic de-allocation (e.g., in case of recursion, deep call stacks,
	/// [permanently-attached](struct.JavaVM.html#attaching-native-threads) native threads, etc.).
	#[deprecated = "Use '.auto()' from IntoAuto trait"]
	pub const fn auto_local<O>(&self, obj: O) -> Auto<'local, O>
	where
		O: Into<JObject<'local>>,
	{
		Auto::new(obj)
	}

	/// Deletes a local reference early, before its JNI stack frame unwinds.
	///
	/// Local references exist within a JNI stack frame, which would typically be created by the
	/// Java VM before making a native method call, and then unwound when your native method
	/// returns.
	///
	/// New JNI stack frames may also be created via [`Self::with_local_frame`].
	///
	/// Typically you don't have to worry about deleting local references since they are
	/// automatically freed when the JNI stack frame they were created in unwinds.
	///
	/// But, each local reference takes memory and so you need to make sure to not excessively
	/// allocate local references.
	///
	/// If you find that you are allocating a large number of local references in a single native
	/// method call, (e.g. while looping over a large collection), this API can be used to
	/// explicitly delete local references before the JNI stack frame unwinds.
	///
	/// In most cases it is better to use [`Auto`] (see [`IntoAuto::auto`] method) or
	/// [`Self::with_local_frame`] instead of directly calling [`Self::delete_local_ref`].
	pub fn delete_local_ref<'other_local, O>(&self, obj: O)
	where
		O: Into<JObject<'other_local>>,
	{
		let obj: JObject<'_> = obj.into();
		let raw: crate::sys::jobject = obj.into_raw();

		// Safety:
		// `raw` may be `null`
		// DeleteLocalRef is safe to call while there may be a pending exception
		unsafe {
			ex_safe_jni_call_no_post_check_ex!(self, v1_1, DeleteLocalRef, raw);
		}
	}
}
