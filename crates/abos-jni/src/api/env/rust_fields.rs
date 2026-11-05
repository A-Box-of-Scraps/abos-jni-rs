use super::*;

impl<'local> Env<'local> {
	/// Looks up the field ID for the given field name and takes the monitor
	/// lock on the given object so the field can be updated without racing
	/// with other Java threads
	pub(super) fn lock_rust_field<'other_local, O, S>(
		&self,
		obj: O,
		field: S,
	) -> Result<(MonitorGuard<'local>, JFieldID)>
	where
		O: AsRef<JObject<'other_local>>,
		S: AsRef<JNIStr>,
	{
		// Note: although the returned Monitor is associated with a lifetime, this API doesn't need
		// a `&mut self` reference because we don't need to create and return a new local reference
		// (Monitors aren't JNI types that are owned by a JNI stack frame).

		// We also don't assert that `self.level == JavaVM::thread_attach_guard_level()` since the
		// returned monitor is associated with the current thread and there's no reason you can't
		// lock with a reference that's not from the top stack frame.

		// Since `Desc::lookup` may need to create a temporary local reference for the object class
		// (which we don't want to leak), we push a new stack frame that we can get a mutable
		// reference for.

		self.with_local_frame(DEFAULT_LOCAL_FRAME_CAPACITY, |env| {
			let obj = obj.as_ref();
			let class = env.get_object_class(obj)?;
			let field_id: JFieldID = Desc::<JFieldID>::lookup((&class, &field, jni_sig!("J")), env)?;
			let guard = self.lock_obj(obj)?;
			Ok((guard, field_id))
		})
	}

	/// Surrenders ownership of a Rust value to Java.
	///
	/// This requires an object with a `long` field to store the pointer.
	///
	/// In Java the property may look like:
	/// ```java
	/// private long myRustValueHandle = 0;
	/// ```
	///
	/// Or, in Kotlin the property may look like:
	/// ```java
	/// private var myRustValueHandle: Long = 0
	/// ```
	///
	/// _Note that `private` properties are accessible to JNI which may be
	/// preferable to avoid exposing the handles to more code than necessary
	/// (since the handles are usually only meaningful to Rust code)_.
	///
	/// The Rust value will be implicitly wrapped in a `Box<Mutex<T>>`.
	///
	/// The Java object will be locked while changing the field value.
	///
	/// # Safety
	///
	/// This will lead to undefined behaviour if the specified field
	/// doesn't have a type of `long`.
	///
	/// It's important to note that using this API will leak memory if
	/// [`Self::take_rust_field`] is never called so that the Rust type may be
	/// dropped.
	///
	/// One suggestion that may help ensure that a set Rust field will be
	/// cleaned up later is for the Java object to implement `Closeable` and let
	/// people use a `use` block (Kotlin) or `try-with-resources` (Java).
	///
	/// **DO NOT** make a copy of the handle stored in one of these fields
	/// since that could lead to a use-after-free error if the Rust type is
	/// taken and dropped multiple times from Rust. If you need to copy an
	/// object with one of these fields then the field should be zero
	/// initialized in the copy.
	pub unsafe fn set_rust_field<'other_local, O, S, T>(&self, obj: O, field: S, rust_object: T) -> Result<()>
	where
		O: AsRef<JObject<'other_local>>,
		S: AsRef<JNIStr>,
		T: Send + 'static,
	{
		let (_guard, field_id) = self.lock_rust_field(&obj, &field)?;

		// It's OK that we don't push a new stack frame here since we know we are dealing with a
		// `jlong` field and since we have already looked up the field ID then we also know that
		// get_field_unchecked and set_field_unchecked don't need to create any local references.

		self.with_top_local_frame(|env| {
			// Safety: the requirement that the given field must be a `long` is
			// documented in the 'Safety' section of this function
			unsafe {
				let field_ptr = env
					.get_field_unchecked(&obj, field_id, ReturnType::Primitive(Primitive::Long))?
					.j()? as *mut Mutex<T>;
				if !field_ptr.is_null() {
					return Err(Error::FieldAlreadySet(field.as_ref().to_str().into()));
				}
			}

			let mbox = Box::new(::std::sync::Mutex::new(rust_object));
			let ptr: *mut Mutex<T> = Box::into_raw(mbox);

			// Safety: the requirement that the given field must be a `long` is
			// documented in the 'Safety' section of this function
			unsafe { env.set_field_unchecked(obj, field_id, (ptr as crate::sys::jlong).into()) }
		})
	}

	/// Gets a lock on a Rust value that's been given to a Java object.
	///
	/// Java still retains ownership and [`Self::take_rust_field`] will still
	/// need to be called at some point.
	///
	/// The Java object will be locked before reading the field value but the
	/// Java object lock will be released after the Rust `Mutex` lock for the
	/// field value has been taken (i.e the Java object won't be locked once
	/// this function returns).
	///
	/// # Safety
	///
	/// This will lead to undefined behaviour if the specified field
	/// doesn't have a type of `long`.
	///
	/// If the field contains a non-zero value then it is assumed to be a valid
	/// pointer that was set via `set_rust_field` and will lead to undefined
	/// behaviour if that is not true.
	pub unsafe fn get_rust_field<'other_local, O, S, T>(&self, obj: O, field: S) -> Result<MutexGuard<'local, T>>
	where
		O: AsRef<JObject<'other_local>>,
		S: AsRef<JNIStr>,
		T: Send + 'static,
	{
		let (_guard, field_id) = self.lock_rust_field(&obj, &field)?;

		// Reference Leaks:
		//
		// It's ok that we don't push a new stack frame here since we know we are dealing with a
		// `jlong` field and since we have already looked up the field ID then we also know that
		// get_field_unchecked doesn't need to create any local references.

		self.with_top_local_frame(|env| {
			// Safety: the requirement that the given field must be a `long` is
			// documented in the 'Safety' section of this function
			unsafe {
				let field_ptr = env
					.get_field_unchecked(obj, field_id, ReturnType::Primitive(Primitive::Long))?
					.j()? as *mut Mutex<T>;
				null_check!(field_ptr, "rust value from Java")?;
				// dereferencing is safe, because we checked it for null
				Ok((*field_ptr).lock().unwrap())
			}
		})
	}

	/// Take a Rust field back from Java.
	///
	/// It sets the field to a null pointer to signal that it's empty.
	///
	/// The Java object will be locked before taking the field value.
	///
	/// # Safety
	///
	/// This will lead to undefined behaviour if the specified field
	/// doesn't have a type of `long`.
	///
	/// If the field contains a non-zero value then it is assumed to be a valid
	/// pointer that was set via `set_rust_field` and will lead to undefined
	/// behaviour if that is not true.
	pub unsafe fn take_rust_field<'other_local, O, S, T>(&self, obj: O, field: S) -> Result<T>
	where
		O: AsRef<JObject<'other_local>>,
		S: AsRef<JNIStr>,
		T: Send + 'static,
	{
		let (_guard, field_id) = self.lock_rust_field(&obj, &field)?;

		// Reference Leaks:
		//
		// It's ok that we don't push a new stack frame here since we know we are dealing with a
		// `jlong` field and since we have already looked up the field ID then we also know that
		// get_field_unchecked doesn't need to create any local references.

		self.with_top_local_frame(|env| {
			// Safety: the requirement that the given field must be a `long` is
			// documented in the 'Safety' section of this function
			let mbox = unsafe {
				let ptr = env
					.get_field_unchecked(&obj, field_id, ReturnType::Primitive(Primitive::Long))?
					.j()? as *mut Mutex<T>;

				null_check!(ptr, "rust value from Java")?;
				Box::from_raw(ptr)
			};

			// attempt to acquire the lock. This prevents us from consuming the
			// mutex if there's an outstanding lock. No one else will be able to
			// get a new one as long as we're in the guarded scope.
			drop(mbox.try_lock()?);

			// Safety: the requirement that the given field must be a `long` is
			// documented in the 'Safety' section of this function
			unsafe {
				env.set_field_unchecked(obj, field_id, (0 as sys::jlong).into())?;
			}

			Ok(mbox.into_inner().unwrap())
		})
	}
}
