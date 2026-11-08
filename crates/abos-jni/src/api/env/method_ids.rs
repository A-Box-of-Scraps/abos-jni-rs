use super::{
	Desc, Env, Error, JClass, JMethodID, JNIStr, JObject, JStaticMethodID, JniError, MethodSignature, Reference, Result,
};

impl<'local> Env<'local> {
	/// Allocates a new object from a class descriptor without running a
	/// constructor.
	///
	/// - Returns [`Error::Instantiation`] if an `InstantiationException`
	///   occurs, if the class is an interface or an abstract class.
	/// - Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an
	///   `OutOfMemoryError` exception occurs.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the class cannot be resolved or instantiated, allocation fails, or a Java exception is pending.
	pub fn alloc_object<'other_local, T>(&mut self, class: T) -> Result<JObject<'local>>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		// SAFETY: The descriptor resolves to a live class; the JNI allocator returns a local object reference.
		let obj: crate::sys::jobject = unsafe {
			jni_call_with_catch_and_null_check!(
                catch |env| {
                    crate::exceptions::JInstantiationException =>
                        Err(Error::Instantiation),
                    crate::exceptions::JOutOfMemoryError =>
                        Err(Error::JniCall(JniError::NoMemory)),
                    else => Err(Error::NullPtr("Unexpected Exception")),
                },
                self, v1_1, AllocObject, class.as_ref().as_raw())?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		// SAFETY: The pointer is a checked local reference allocated in the current frame.
		Ok(unsafe { JObject::from_raw(self, obj) })
	}

	/// Look up a method by class descriptor, name, and signature.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JMethodID};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let method_id: JMethodID =
	///     env.get_method_id(jni_str!("java/lang/String"), jni_str!("substring"), jni_sig!("(II)Ljava/lang/String;"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Returns a valid method ID on success or [`Error::MethodNotFound`] on failure.
	///
	/// Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an `OutOfMemoryError` is thrown by
	/// the JVM.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the class or method cannot be resolved, its signature does not match, or a Java exception is pending.
	///
	/// Use [`Self::get_method_id_from_class`] with an already-resolved class and a shared environment.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null method ID without reporting an exception.
	pub fn get_method_id<'other_local, 'sig, 'sig_args, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
	) -> Result<JMethodID>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		self.get_method_id_from_class(class.as_ref(), name, sig)
	}

	/// Looks up an instance method on an already-resolved class using a shared environment.
	///
	/// See [`Self::get_method_id`] for lookup and exception behavior.
	///
	/// # Errors
	///
	/// Returns an error if the class is null, lookup fails, or a Java exception is pending.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null ID without reporting an exception.
	pub fn get_method_id_from_class<'sig, 'sig_args, N, S>(
		&self,
		class: &JClass<'_>,
		name: N,
		sig: S,
	) -> Result<JMethodID>
	where
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		let class: &JClass<'_> = null_check!(class, "get_method_id_from_class class")?;
		let ffi_name: &JNIStr = name.as_ref();
		let sig: &MethodSignature<'_, '_> = sig.as_ref();

		// SAFETY: The resolved class, method name and signature remain valid during the JNI lookup.
		let method_id: crate::sys::jmethodID = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchMethodError =>
						Err(Error::MethodNotFound {
							name: ffi_name.to_string(),
							sig: sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception: std::result::Result<crate::objects::JThrowable<'_>, crate::errors::Error> = e.get_exception(env);
						let exception: Option<crate::refs::Global<crate::objects::JThrowable<'_>>> = exception.map_or_else(|_| None, |exception| env.new_global_ref(exception).ok());
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetMethodID,
				class.as_raw(),
				ffi_name.as_ptr(),
				sig.as_ref().sig().as_ptr()
			)?
		};

		assert!(
			!method_id.is_null(),
			"Spurious null method ID returned, without a pending exception"
		);

		// SAFETY: The method ID came from JNI and was checked for null.
		Ok(unsafe { JMethodID::from_raw(method_id) })
	}

	/// Look up a static method by class descriptor, name, and signature.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JStaticMethodID};
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let method_id: JStaticMethodID =
	///     env.get_static_method_id(jni_str!("java/lang/String"), jni_str!("valueOf"), jni_sig!("(I)Ljava/lang/String;"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Returns a valid method ID on success or [`Error::MethodNotFound`] on failure.
	///
	/// Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an `OutOfMemoryError` is thrown by
	/// the JVM.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the class or method cannot be resolved, its signature does not match, or a Java exception is pending.
	///
	/// Use [`Self::get_static_method_id_from_class`] with an already-resolved class and a shared environment.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null method ID without reporting an exception.
	pub fn get_static_method_id<'other_local, 'sig, 'sig_args, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
	) -> Result<JStaticMethodID>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		self.get_static_method_id_from_class(class.as_ref(), name, sig)
	}

	/// Looks up a static method on an already-resolved class using a shared environment.
	///
	/// See [`Self::get_static_method_id`] for lookup and exception behavior.
	///
	/// # Errors
	///
	/// Returns an error if the class is null, lookup fails, or a Java exception is pending.
	///
	/// # Panics
	///
	/// Panics if JNI returns a null ID without reporting an exception.
	pub fn get_static_method_id_from_class<'sig, 'sig_args, N, S>(
		&self,
		class: &JClass<'_>,
		name: N,
		sig: S,
	) -> Result<JStaticMethodID>
	where
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		let class: &JClass<'_> = null_check!(class, "get_static_method_id_from_class class")?;
		let ffi_name: &JNIStr = name.as_ref();
		let sig: &MethodSignature<'_, '_> = sig.as_ref();

		// SAFETY: The resolved class, method name and signature remain valid during the JNI lookup.
		let method_id: crate::sys::jmethodID = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchMethodError =>
						Err(Error::MethodNotFound {
							name: ffi_name.to_string(),
							sig: sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception: std::result::Result<crate::objects::JThrowable<'_>, crate::errors::Error> = e.get_exception(env);
						let exception: Option<crate::refs::Global<crate::objects::JThrowable<'_>>> = exception.map_or_else(|_| None, |exception| env.new_global_ref(exception).ok());
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetStaticMethodID,
				class.as_raw(),
				ffi_name.as_ptr(),
				sig.as_ref().sig().as_ptr()
			)?
		};

		assert!(
			!method_id.is_null(),
			"Spurious null method ID returned, without a pending exception"
		);

		// SAFETY: The static method ID came from JNI and was checked for null.
		Ok(unsafe { JStaticMethodID::from_raw(method_id) })
	}
}
