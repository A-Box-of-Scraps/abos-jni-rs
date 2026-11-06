use super::{Env, Error, Global, JObject, JniError, Reference, Result, Weak, sys};

impl Env<'_> {
	/// Creates a new global reference to the Java object `obj`.
	///
	/// Global references take more time to create or delete than ordinary
	/// local references do, but have several properties that make them useful
	/// in certain situations. See [`Global`] for more information.
	///
	/// If you use this API to try and upgrade a [`Weak`] then it may return
	/// [`Error::ObjectFreed`] if the object has been garbage collected.
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate a global reference or a weak source reference has already been collected.
	pub fn new_global_ref<'any_local, O>(&self, obj: O) -> Result<Global<O::GlobalKind>>
	where
		O: Reference + AsRef<JObject<'any_local>>,
	{
		// Avoid passing null to `NewGlobalRef` so that we can recognise out-of-memory errors
		if obj.is_null() {
			return Ok(Global::null());
		}

		// Safety:
		// - the minimum supported JNI version is 1.4
		// - we can assume that `obj.raw()` is a valid reference
		// - we know there's no other wrapper for the reference passed to from_global_raw
		//   since we have just created it.
		let global_ref: Global<O::GlobalKind> = unsafe {
			let global_ref: O::GlobalKind =
				O::global_kind_from_raw(jni_call_no_post_check_ex!(self, v1_1, NewGlobalRef, obj.as_raw())?);
			Global::new(self, global_ref)
		};

		// Per JNI spec, `NewGlobalRef` will return a null pointer if the object was GC'd
		// (which could happen if `obj` is a `Weak`):
		//
		//  > it is recommended that a (strong) local or global reference to the
		//  > underlying object be acquired using one of the JNI functions
		//  > NewLocalRef or NewGlobalRef. These functions will return NULL if
		//  > the object has been freed.
		//
		if global_ref.is_null() {
			// In this case it's ambiguous whether there has been an out-of-memory error or
			// the object has been garbage collected and so we now _explicitly_ check
			// whether the object has been garbage collected.
			if self.is_same_object(obj, JObject::null())? {
				Err(Error::ObjectFreed)
			} else {
				Err(Error::JniCall(JniError::NoMemory))
			}
		} else {
			Ok(global_ref)
		}
	}

	/// Creates a new global reference and casts it to a different type.
	///
	/// This is a convenience method that combines [`Self::cast_global`] and
	/// [`Self::new_global_ref`].
	///
	/// It first checks if the object is an instance of the target type, and if so it creates a new
	/// global reference with the target type.
	///
	/// `obj` can be a local reference or a global reference.
	///
	/// For upcasting (converting to a more general type), consider using the `AsRef` trait
	/// implementations instead, which don't require runtime checks.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let local_obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let global_string = env.new_cast_global_ref::<JString>(local_obj)?;
	/// // global_string is now a `Global<JString>` that persists beyond local frames
	///
	/// // For upcasting, the `AsRef` trait is more efficient:
	/// let as_obj_again: &JObject = global_string.as_ref(); // No runtime check needed
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of the target type.
	/// Returns [`Error::ClassNotFound`] if the target class cannot be found.
	pub fn new_cast_global_ref<'any_local, To>(
		&self,
		obj: impl Reference + AsRef<JObject<'any_local>>,
	) -> Result<Global<To::GlobalKind>>
	where
		To: Reference,
	{
		if obj.is_null() {
			return Ok(Global::default());
		}

		if self.is_instance_of_cast_type::<To>(obj.as_ref())? {
			let raw: crate::sys::jobject = self.new_global_ref(obj)?.into_raw();
			// Safety:
			// - we have just checked that the global reference is an instance of `To`
			unsafe {
				let cast: To::GlobalKind = To::global_kind_from_raw(raw);
				Ok(Global::new(self, cast))
			}
		} else {
			Err(Error::WrongObjectType)
		}
	}

	/// Creates an auto-delete [`Global`] reference wrapper around a raw JNI `jobject` pointer.
	///
	/// This may be useful if a third-party library gives you ownership over a JNI global reference
	/// pointer and you want to manage it safely in Rust.
	///
	/// If you don't own the global reference you must not use this function and can instead
	/// use [`Env::as_cast_raw`] to temporarily wrap the raw pointer without taking ownership.
	///
	/// # Safety
	/// - The `raw` pointer must be a valid JNI reference of the appropriate type `T`, or `null`.
	/// - The `raw` pointer must not have any other existing wrappers
	/// - You must logically own the global reference and be able to guarantee that it can not
	///   be deleted elsewhere while the returned [`Global`] is alive.
	pub unsafe fn global_from_raw<T: Reference>(&self, raw: sys::jobject) -> Global<T::GlobalKind> {
		// Safety: the rules above cover the safety requirements
		unsafe { Global::new(self, T::global_kind_from_raw(raw)) }
	}

	/// Attempts to cast an owned global reference to a different type.
	///
	/// This performs a runtime type check using `IsInstanceOf` and consumes the input reference.
	///
	/// For upcasting (converting to a more general type), consider using the `AsRef` trait
	/// implementations instead, which don't require runtime checks.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let local_obj: JObject = env.new_object(jni_str!("java/lang/String"), jni_sig!("()V"), &[])?;
	/// let global_obj: Global<JObject<'static>> = env.new_global_ref(&local_obj)?;
	/// let global_string = env.cast_global::<JString>(global_obj)?;
	///
	/// // For upcasting, the `AsRef` trait is more efficient:
	/// let as_obj_again: &JObject = global_string.as_ref(); // No runtime check needed
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the object is not an instance of the target type.
	/// Returns [`Error::ClassNotFound`] if the target class cannot be found.
	pub fn cast_global<To>(
		&self,
		obj: Global<
			impl Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
		>,
	) -> Result<Global<To::GlobalKind>>
	where
		To: Reference,
	{
		if obj.is_null() {
			return Ok(Global::default());
		}

		if self.is_instance_of_cast_type::<To>(obj.as_ref())? {
			// Safety:
			// - we have just checked that `obj` is an instance of `T`
			// - there won't be multiple wrappers since we are creating one from the other
			unsafe {
				let cast: To::GlobalKind = To::global_kind_from_raw(obj.into_raw());
				Ok(Global::new(self, cast))
			}
		} else {
			Err(Error::WrongObjectType)
		}
	}

	/// Creates a new weak global reference.
	///
	/// Weak global references are a special kind of Java object reference that
	/// doesn't prevent the Java object from being garbage collected. See
	/// [`Weak`] for more information.
	///
	/// If you use this API to create a [`Weak`] from another [`Weak`]
	/// then it may return [`Error::ObjectFreed`] if the object has been garbage
	/// collected.
	///
	/// Attempting to create a [`Weak`] for a `null` reference will return an
	/// [`Error::ObjectFreed`] error.
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate a weak global reference or a weak source reference has already been collected.
	pub fn new_weak_ref<O>(&self, obj: O) -> Result<Weak<O::GlobalKind>>
	where
		O: Reference,
	{
		// Check if the pointer is null *before* calling `NewWeakGlobalRef`.
		//
		// This avoids a bug in some JVM implementations which, contrary to the JNI specification,
		// will throw `java.lang.OutOfMemoryError: C heap space` from `NewWeakGlobalRef` if it is
		// passed a null pointer. (The specification says it will return a null pointer in that
		// situation, not throw an exception.)
		if obj.is_null() {
			return Err(Error::ObjectFreed);
		}

		// Safety:
		// - the minimum supported JNI version is 1.4
		// - we can assume that `obj.raw()` is a valid reference
		// - we know there's no other wrapper for the reference passed to from_global_raw
		//   since we have just created it.
		let weak_ref: Weak<O::GlobalKind> = unsafe {
			let weak: O::GlobalKind = O::global_kind_from_raw(jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_2,
				NewWeakGlobalRef,
				obj.as_raw()
			)?);
			Weak::new(self, weak)
		};

		// Unlike for NewLocalRef and NewGlobalRef, the JNI spec doesn't seem to
		// give the same guarantee that it will return null if the object has
		// already been freed, but it seems reasonable to assume it can.

		if weak_ref.is_null() {
			// Unlike for NewLocalRef and NewGlobalRef we can assume that NewWeakGlobalRef
			// will throw an out-of-memory exception (that we catch) instead of returning null
			Err(Error::ObjectFreed)
		} else {
			Ok(weak_ref)
		}
	}

	/// Creates an auto-delete [`Weak`] reference wrapper around a raw JNI `jobject` pointer.
	///
	/// This may be useful if a third-party library gives you ownership over a JNI weak reference
	/// pointer and you want to manage it safely in Rust.
	///
	/// If you don't own the weak reference you must not use this function and can instead
	/// use [`Env::as_cast_raw`] to temporarily wrap the raw pointer without taking ownership.
	///
	/// # Safety
	/// - The `raw` pointer must be a valid JNI reference of the appropriate type `T`, or `null`.
	/// - The `raw` pointer must not have any other existing wrappers
	/// - You must logically own the weak reference and be able to guarantee that it can not
	///   be deleted elsewhere while the returned [`Weak`] is alive.
	pub unsafe fn weak_from_raw<T: Reference>(&self, raw: sys::jobject) -> Weak<T::GlobalKind> {
		// Safety: the rules above cover the safety requirements
		unsafe { Weak::new(self, T::global_kind_from_raw(raw)) }
	}
}
