use super::{
	AutoElements, AutoElementsCritical, Desc, Env, Error, JClass, JObject, JPrimitiveArray, JavaVM, JniError,
	MonitorGuard, NativeMethod, Reference, ReleaseMode, Result, TryInto, TypeArray, jint, jni_error_code_to_result,
	sys,
};

impl<'local> Env<'local> {
	/// Lock a Java object. The `MonitorGuard` that this returns is responsible
	/// for ensuring that it gets unlocked.
	///
	/// The guard borrows `obj` and cannot outlive either that reference or this environment's
	/// thread attachment. Drop the guard before deleting a local reference or dropping its
	/// [`Global`](crate::refs::Global) or [`Auto`](crate::refs::Auto) owner.
	/// No additional JNI reference is allocated, and `Env` can still be used mutably while locked.
	///
	/// # Errors
	///
	/// Returns an error if the object is null, the monitor cannot be acquired, or a Java exception is pending.
	pub fn lock_obj<'guard, 'other_local, O>(&self, obj: &'guard O) -> Result<MonitorGuard<'guard>>
	where
		'local: 'guard,
		'other_local: 'guard,
		O: AsRef<JObject<'other_local>> + ?Sized,
	{
		// Note: although the returned Monitor is associated with a lifetime, we
		// don't need a `&mut self` reference and we don't assert that
		// `self.level == JavaVM::thread_attach_guard_level()` since the
		// returned monitor is associated with the current thread and is not a
		// local reference.

		let obj: &'guard JObject<'other_local> = null_check!(obj.as_ref(), "lock_obj obj argument")?;
		// SAFETY: The non-null reference and thread attachment outlive the guard, which releases the monitor.
		let res: i32 = unsafe { jni_call_no_post_check_ex!(self, v1_1, MonitorEnter, obj.as_raw())? };
		jni_error_code_to_result(res)?;

		Ok(MonitorGuard {
			obj,
			_not_send_sync: std::marker::PhantomData,
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
		let res: i32 = unsafe { jni_call_no_post_check_ex!(self, v1_2, EnsureLocalCapacity, capacity)? };
		jni_error_code_to_result(res)?;
		Ok(())
	}

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
	/// `unsafe` because registration also requires the correct static/instance receiver and
	/// a code lifetime that covers all possible calls.
	///
	/// - **Static native methods** must have a `class: JClass<'local>` parameter as the second parameter.
	///
	/// - **Instance native methods** must have a `this: JObject<'local>` or `this: MyType<'local>` parameter as the second parameter.
	///
	/// The registered code must remain executable for as long as the JVM can call it, including
	/// until all in-flight calls have returned. In particular, the caller must keep any native
	/// library containing that code loaded for this entire period. The lifetime of the `methods`
	/// slice or its descriptors does not bound the registration lifetime. This API does not retain
	/// the library, and dropping the descriptors does not unregister the methods.
	///
	/// A global reference to the target class is not a general library keepalive: the library may
	/// belong to a different class loader, which that reference does not keep alive. The caller
	/// must retain the library's owning loader or otherwise prevent library unloading.
	///
	/// Before unloading, prevent new calls and concurrent registration, remove the registrations,
	/// and ensure that all in-flight calls have returned. Unregistration alone, including from a
	/// destructor, does not establish this synchronization. These obligations also apply to any
	/// registrations already installed when registration or later initialization returns an error.
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
	/// Use [`Self::register_native_methods_from_class`] with an already-resolved class and a shared environment.
	///
	/// # Errors
	///
	/// Returns an error if the class cannot be resolved or the JVM rejects a native method name or signature.
	pub unsafe fn register_native_methods<'other_local, T>(&mut self, class: T, methods: &[NativeMethod]) -> Result<()>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		// SAFETY: The caller guarantees the native method contracts; the descriptor stays live.
		unsafe { self.register_native_methods_from_class(class.as_ref(), methods) }
	}

	/// Registers native methods on an already-resolved class using a shared environment.
	///
	/// # Safety
	///
	/// The caller must satisfy all requirements of [`Self::register_native_methods`], including
	/// function-pointer validity, matching signatures and static/instance receivers, and keeping
	/// the registered code and its native library alive until no new calls can occur and all
	/// in-flight calls have returned.
	///
	/// # Errors
	///
	/// Returns an error if the class is null, JNI rejects the methods, or a Java exception is pending.
	pub unsafe fn register_native_methods_from_class(
		&self,
		class: &JClass<'_>,
		methods: &[NativeMethod],
	) -> Result<()> {
		let class: &JClass<'_> = null_check!(class, "register_native_methods_from_class class")?;
		// Safety:
		//  - NativeMethod is a #[transparent] / repr(C)] wrapper around sys::JNINativeMethod
		//  - NativeMethod only has an `unsafe` constructor that requires the caller to ensure that
		//    the function pointer is valid and matches the signature
		//  - The caller guarantees the receiver kind and keeps the code executable for all calls,
		//    including in-flight calls after unregistration.
		let res: i32 = unsafe {
			jni_call_with_catch!(
				catch |env| {
					e: crate::exceptions::JNoSuchMethodError => {
						let msg: Option<crate::objects::JString<'_>> = e.as_throwable().get_message(env).ok();
						let msg: String = msg.map_or_else(String::new, |msg| msg.to_string());
						Err(Error::NoSuchMethod(msg))
					},
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				RegisterNatives,
				class.as_raw(),
				methods.as_ptr().cast::<sys::JNINativeMethod>().cast_mut(),
				jint::try_from(methods.len()).map_err(|_| Error::JniCall(JniError::InvalidArguments))?
			)?
		};

		jni_error_code_to_result(res)
	}

	/// Unbind all native methods of class.
	///
	/// This is not a synchronization barrier for native calls and does not guarantee that
	/// in-flight calls have returned. Calling this method, even from a destructor, is not by
	/// itself sufficient to safely unload a native library. The application must also prevent
	/// new calls and concurrent registration and wait for all in-flight calls to return before
	/// unloading code. See the lifetime requirements of [`Self::register_native_methods`].
	///
	/// # Errors
	///
	/// Returns an error if the class is null or cannot be resolved, unregistration fails, or a Java exception is pending.
	pub fn unregister_native_methods<'other_local, T>(&mut self, class: T) -> Result<()>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let resolved_class: &JClass<'_> = class.as_ref();
		let resolved_class: &JClass<'_> = null_check!(resolved_class, "unregister_native_methods class argument")?;
		// SAFETY: The class is non-null and its descriptor remains live during UnregisterNatives.
		let res: i32 = unsafe { jni_call_post_check_ex!(self, v1_1, UnregisterNatives, resolved_class.as_raw())? };

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
