use super::{
	AutoElements, AutoElementsCritical, Desc, Env, Error, JClass, JObject, JPrimitiveArray, JavaVM, JniError,
	MonitorGuard, NativeMethod, Reference, ReleaseMode, Result, TryInto, TypeArray, jint, jni_error_code_to_result,
	sys,
};

impl<'local> Env<'local> {
	/// Lock a Java object. The `MonitorGuard` that this returns is responsible
	/// for ensuring that it gets unlocked.
	///
	/// # Errors
	///
	/// Returns an error if the object is null, the monitor cannot be acquired, or a Java exception is pending.
	pub fn lock_obj<'other_local, O>(&self, obj: O) -> Result<MonitorGuard<'local>>
	where
		O: AsRef<JObject<'other_local>>,
	{
		// Note: although the returned Monitor is associated with a lifetime, we
		// don't need a `&mut self` reference and we don't assert that
		// `self.level == JavaVM::thread_attach_guard_level()` since the
		// returned monitor is associated with the current thread and is not a
		// local reference.

		let inner = obj.as_ref().as_raw();
		// SAFETY: The object reference is non-null and live; MonitorGuard will release the acquired monitor.
		let res = unsafe { jni_call_no_post_check_ex!(self, v1_1, MonitorEnter, inner)? };
		jni_error_code_to_result(res)?;

		Ok(MonitorGuard {
			obj: inner,
			life: std::marker::PhantomData,
		})
	}

	/// Returns the Java VM interface.
	///
	/// May return [`Error::JavaException`] if called while there is a pending
	/// exception.
	///
	/// # Errors
	///
	/// Returns an error if JNI cannot obtain the VM or a Java exception is pending.
	pub fn get_java_vm(&self) -> Result<JavaVM> {
		// This avoids calling JNI if we already know the VM pointer
		JavaVM::from_env(self)
	}

	/// Ensures that at least a given number of local references can be created
	/// in the current thread.
	///
	/// # Errors
	///
	/// Returns an error if the capacity does not fit JNI, sufficient local-reference capacity cannot be reserved, or a Java exception is pending.
	pub fn ensure_local_capacity(&self, capacity: usize) -> Result<()> {
		let capacity: jint = capacity
			.try_into()
			.map_err(|_| Error::JniCall(JniError::InvalidArguments))?;
		// Safety:
		// - jni-rs required JNI_VERSION > 1.2
		// - we have ensured capacity is >= 0
		// - EnsureLocalCapacity has no documented exceptions that it throws
		let res = unsafe { jni_call_no_post_check_ex!(self, v1_2, EnsureLocalCapacity, capacity)? };
		jni_error_code_to_result(res)?;
		Ok(())
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
	/// Bind function pointers to native methods of class according to method name and signature.
	///
	/// For details see
	/// [documentation](https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/functions.html#RegisterNatives).
	///
	/// # Safety
	///
	/// The caller must ensure that the function signatures in `methods` have a `this` or `class`
	/// parameter as appropriate for instance or static methods, and that the function pointers are
	/// valid and match the signatures.
	///
	/// Although `NativeMethod` has an `unsafe` constructor that requires the caller to ensure that
	/// the function pointer is valid and matches the signature, this API is still marked as
	/// `unsafe` to reflect the additional risk that the signature matches but the second parameter
	/// is not consistent with whether the method is static or instance.
	///
	/// - **Static native methods** must have a `class: JClass<'local>` parameter as the second parameter.
	///
	/// - **Instance native methods** must have a `this: JObject<'local>` or `this: MyType<'local>` parameter as the second parameter.
	///
	/// # Throws
	///
	/// - `NoSuchMethodError` - if a method could not be found (based on its name and signature) or
	///   it was not a native method.
	///
	/// **Note** that there is no way for the JVM to report errors if the function pointer is invalid or
	/// does not match the signature, so incorrect function pointers may lead to crashes or
	/// undefined behaviour.
	///
	/// # Errors
	///
	/// Returns an error if the class cannot be resolved or the JVM rejects a native method name or signature.
	pub unsafe fn register_native_methods<'other_local, T>(&mut self, class: T, methods: &[NativeMethod]) -> Result<()>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		let class = class.lookup(self)?;
		// Safety:
		//  - NativeMethod is a #[transparent] / repr(C)] wrapper around sys::JNINativeMethod
		//  - NativeMethod only has an `unsafe` constructor that requires the caller to ensure that
		//    the function pointer is valid and matches the signature
		let res = unsafe {
			jni_call_with_catch!(
				catch |env| {
					e: crate::exceptions::JNoSuchMethodError => {
						let msg = e.as_throwable().get_message(env).ok();
						let msg = msg.map_or_else(String::new, |msg| msg.to_string());
						Err(Error::NoSuchMethod(msg))
					},
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				RegisterNatives,
				class.as_ref().as_raw(),
				methods.as_ptr().cast::<sys::JNINativeMethod>().cast_mut(),
				jint::try_from(methods.len()).map_err(|_| Error::JniCall(JniError::InvalidArguments))?
			)?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		jni_error_code_to_result(res)
	}

	/// Unbind all native methods of class.
	///
	/// # Errors
	///
	/// Returns an error if the class cannot be resolved, unregistration fails, or a Java exception is pending.
	pub fn unregister_native_methods<'other_local, T>(&mut self, class: T) -> Result<()>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		let class = class.lookup(self)?;
		// SAFETY: The class descriptor remains live during UnregisterNatives.
		let res = unsafe { jni_call_post_check_ex!(self, v1_1, UnregisterNatives, class.as_ref().as_raw())? };

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		jni_error_code_to_result(res)
	}

	/// Returns an [`AutoElements`] to access the elements of the given Java `array`.
	///
	/// # Safety
	///
	/// See: [`JPrimitiveArray::get_elements`] for more details
	#[deprecated(
		since = "0.22.0",
		note = "use JPrimitiveArray::get_elements instead. This API will be removed in a future version"
	)]
	///
	/// # Errors
	///
	/// Returns an error if the array is null, element acquisition fails, or a Java exception is pending.
	pub unsafe fn get_array_elements<'array_local, T, TArrayRef>(
		&self,
		array: TArrayRef,
		mode: ReleaseMode,
	) -> Result<AutoElements<'array_local, T, TArrayRef>>
	where
		T: TypeArray,
		TArrayRef: AsRef<JPrimitiveArray<'array_local, T>> + Reference,
	{
		AutoElements::new(self, array, mode)
	}

	/// Returns an [`AutoElementsCritical`] to access the elements of the given Java `array`.
	///
	/// # Safety
	///
	/// See: [`JPrimitiveArray::get_elements_critical`] for more details
	#[deprecated(
		since = "0.22.0",
		note = "use JPrimitiveArray::get_elements_critical instead. This API will be removed in a future version"
	)]
	///
	/// # Errors
	///
	/// Returns an error if the array is null, critical element acquisition fails, or a Java exception is pending.
	pub unsafe fn get_array_elements_critical<'array_local, T, TArrayRef>(
		&self,
		array: TArrayRef,
		mode: ReleaseMode,
	) -> Result<AutoElementsCritical<'array_local, T, TArrayRef>>
	where
		T: TypeArray,
		TArrayRef: AsRef<JPrimitiveArray<'array_local, T>> + Reference,
	{
		AutoElementsCritical::new(self, array, mode)
	}
}
