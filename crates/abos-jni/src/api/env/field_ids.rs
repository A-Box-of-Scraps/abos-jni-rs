use super::{
	Desc, Env, Error, FieldSignature, JClass, JFieldID, JNIStr, JObject, JStaticFieldID, JniError, Reference, Result,
};

impl<'local> Env<'local> {
	/// Look up the field ID for a class/name/type combination.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JFieldID};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let field_id: JFieldID = env.get_field_id(jni_str!("com/my/Class"), jni_str!("intField"), jni_sig!("I"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Returns a valid field ID on success or [`Error::FieldNotFound`] on failure.
	///
	/// Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an `OutOfMemoryError` is thrown by
	/// the JVM.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the class or field cannot be resolved, its signature does not match, or a Java exception is pending.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null field ID without reporting an exception.
	pub fn get_field_id<'other_local, 'sig, C, N, S>(&mut self, class: C, name: N, sig: S) -> Result<JFieldID>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<FieldSignature<'sig>>,
	{
		let class = class.lookup(self)?;
		let ffi_name = name.as_ref();
		let ffi_sig = sig.as_ref();

		// SAFETY: The resolved class, field name and signature remain valid during the JNI lookup.
		let field_id = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchFieldError =>
						Err(Error::FieldNotFound {
							name: ffi_name.to_string(),
							sig: ffi_sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception = e.get_exception(env);
						let exception = exception.map_or_else(|_| None, |exception| env.new_global_ref(exception).ok());
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetFieldID,
				class.as_ref().as_raw(),
				ffi_name.as_ptr(),
				ffi_sig.sig().as_ptr()
			)?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		assert!(!field_id.is_null(), "Spurious null field ID returned with no exception");

		// SAFETY: The field ID came from JNI and was checked for null.
		Ok(unsafe { JFieldID::from_raw(field_id) })
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
	/// Look up the static field ID for a class/name/type combination.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JStaticFieldID};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let field_id: JStaticFieldID = env.get_static_field_id(jni_str!("com/my/Class"), jni_str!("intField"), jni_sig!("I"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Returns a valid field ID on success or [`Error::FieldNotFound`] on failure.
	///
	/// Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an `OutOfMemoryError` is thrown by
	/// the JVM.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the class or field cannot be resolved, its signature does not match, or a Java exception is pending.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null field ID without reporting an exception.
	pub fn get_static_field_id<'other_local, 'sig, T, U, V>(
		&mut self,
		class: T,
		name: U,
		sig: V,
	) -> Result<JStaticFieldID>
	where
		T: Desc<'local, JClass<'other_local>>,
		U: AsRef<JNIStr>,
		V: AsRef<FieldSignature<'sig>>,
	{
		let class = class.lookup(self)?;
		let ffi_name = name.as_ref();
		let ffi_sig = sig.as_ref();

		// SAFETY: The resolved class, field name and signature remain valid during the JNI lookup.
		let field_id = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchFieldError =>
						Err(Error::FieldNotFound {
							name: ffi_name.to_string(),
							sig: ffi_sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception = e.get_exception(env);
						let exception = exception.map_or_else(|_| None, |exception| env.new_global_ref(exception).ok());
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetStaticFieldID,
				class.as_ref().as_raw(),
				ffi_name.as_ptr(),
				ffi_sig.sig().as_ptr()
			)?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		assert!(!field_id.is_null(), "Spurious null field ID returned with no exception");

		// SAFETY: The static field ID came from JNI and was checked for null.
		Ok(unsafe { JStaticFieldID::from_raw(field_id) })
	}

	/// Get the class for an object.
	///
	/// # Errors
	///
	/// Returns an error if the object is null or a Java exception is pending.
	pub fn get_object_class<'other_local, O>(&mut self, obj: O) -> Result<JClass<'local>>
	where
		O: AsRef<JObject<'other_local>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let obj = obj.as_ref();
		let obj = null_check!(obj, "get_object_class")?;
		// SAFETY: The object is non-null and valid; GetObjectClass returns a local reference in this frame.
		unsafe {
			Ok(JClass::from_raw(
				self,
				jni_call_no_post_check_ex!(self, v1_1, GetObjectClass, obj.as_raw())?,
			))
		}
	}
}
