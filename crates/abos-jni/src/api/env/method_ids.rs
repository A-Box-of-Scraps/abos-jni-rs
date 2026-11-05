use super::*;

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
	pub fn alloc_object<'other_local, T>(&mut self, class: T) -> Result<JObject<'local>>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class = class.lookup(self)?;
		let obj = unsafe {
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

		Ok(unsafe { JObject::from_raw(self, obj) })
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
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
		let class = class.lookup(self)?;
		let ffi_name = name.as_ref();
		let sig = sig.as_ref();

		let method_id = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchMethodError =>
						Err(Error::MethodNotFound {
							name: ffi_name.to_string(),
							sig: sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception = e.get_exception(env);
						let exception = if let Ok(exception) = exception {
							env.new_global_ref(exception).ok()
						} else {
							None
						};
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetMethodID,
				class.as_ref().as_raw(),
				ffi_name.as_ptr(),
				sig.as_ref().sig().as_ptr()
			)?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		assert!(
			!method_id.is_null(),
			"Spurious null method ID returned, without a pending exception"
		);

		Ok(unsafe { JMethodID::from_raw(method_id) })
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
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
		let class = class.lookup(self)?;
		let ffi_name = name.as_ref();
		let sig = sig.as_ref();

		let method_id = unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JNoSuchMethodError =>
						Err(Error::MethodNotFound {
							name: ffi_name.to_string(),
							sig: sig.sig().to_string(),
						}),
					e: crate::exceptions::JExceptionInInitializerError => {
						let exception = e.get_exception(env);
						let exception = if let Ok(exception) = exception {
							env.new_global_ref(exception).ok()
						} else {
							None
						};
						Err(Error::ExceptionInInitializer { exception })
					},
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetStaticMethodID,
				class.as_ref().as_raw(),
				ffi_name.as_ptr(),
				sig.as_ref().sig().as_ptr()
			)?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		assert!(
			!method_id.is_null(),
			"Spurious null method ID returned, without a pending exception"
		);

		Ok(unsafe { JStaticMethodID::from_raw(method_id) })
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
}
