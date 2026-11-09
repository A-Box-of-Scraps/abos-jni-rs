use crate::sys;

use super::{Desc, Env, JClass, JMethodID, JNIStr, JObject, JStaticMethodID, MethodSignature, Reference, Result};

impl<'local> Env<'local> {
	/// Looks up an instance method or constructor and returns its
	/// `java.lang.reflect.Method` or `java.lang.reflect.Constructor` object.
	///
	/// The method is resolved against the supplied class, which is kept alive
	/// through reflection. Use `<init>` as the name and a void return signature
	/// for constructors. Inherited methods are supported.
	///
	/// To reflect a cached method ID, use [`Self::to_reflected_method_unchecked`].
	///
	/// # Errors
	///
	/// Returns an error if class or method lookup fails, or JNI cannot create the reflected object.
	/// See [`Self::get_method_id`] for method lookup errors.
	pub fn to_reflected_method<'other_local, 'sig, 'sig_args, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
	) -> Result<JObject<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		self.assert_top();
		let class: C::Output = class.lookup(self)?;
		let method_id: JMethodID = self.get_method_id_from_class(class.as_ref(), name, sig)?;
		// SAFETY: The instance method ID was resolved against this same class, which remains live until reflection returns.
		let reflected: Result<JObject<'local>> =
			unsafe { self.to_reflected_method_unchecked(class.as_ref(), method_id) };
		drop(class);
		reflected
	}

	/// Looks up a static method and returns its `java.lang.reflect.Method` object.
	///
	/// The method is resolved against the supplied class, which is kept alive
	/// through reflection. Inherited static methods are supported.
	///
	/// To reflect a cached method ID, use [`Self::to_reflected_static_method_unchecked`].
	///
	/// # Errors
	///
	/// Returns an error if class or method lookup fails, or JNI cannot create the reflected object.
	/// See [`Self::get_static_method_id`] for method lookup errors.
	pub fn to_reflected_static_method<'other_local, 'sig, 'sig_args, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
	) -> Result<JObject<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		self.assert_top();
		let class: C::Output = class.lookup(self)?;
		let method_id: JStaticMethodID = self.get_static_method_id_from_class(class.as_ref(), name, sig)?;
		// SAFETY: The static method ID was resolved against this same class, which remains live until reflection returns.
		let reflected: Result<JObject<'local>> =
			unsafe { self.to_reflected_static_method_unchecked(class.as_ref(), method_id) };
		drop(class);
		reflected
	}

	/// Convert a [`JMethodID`] into a [`JObject`] with the corresponding
	/// `java.lang.reflect.Method` or `java.lang.reflect.Constructor` instance.
	///
	/// Prefer [`Self::to_reflected_method`] when the method name and signature are available.
	///
	/// # Safety
	///
	/// The resolved class must be non-null, and the method ID must be valid for
	/// that class, as if obtained by looking up the method against that class.
	/// The ID must identify an instance method or constructor, and its declaring
	/// class must remain loaded until this call returns.
	/// Resolving the descriptors does not verify their compatibility.
	///
	/// # Errors
	///
	/// Returns an error if resolving the class or method descriptor fails, or JNI cannot create the reflected object.
	pub unsafe fn to_reflected_method_unchecked<'other_local>(
		&mut self,
		class: impl Desc<'local, JClass<'other_local>>,
		method_id: impl Desc<'local, JMethodID>,
	) -> Result<JObject<'local>> {
		// SAFETY: The caller guarantees class compatibility and ID validity; JMethodID identifies an instance method.
		unsafe { self.to_reflected_method_base(class, method_id, JMethodID::into_raw, false) }
	}

	/// Convert a [`JStaticMethodID`] into a [`JObject`] with the corresponding
	/// `java.lang.reflect.Method` instance.
	///
	/// Prefer [`Self::to_reflected_static_method`] when the method name and signature are available.
	///
	/// # Safety
	///
	/// The resolved class must be non-null, and the method ID must be valid for
	/// that class, as if obtained by looking up the method against that class.
	/// The ID must identify a static method, and its declaring class must remain
	/// loaded until this call returns.
	/// Resolving the descriptors does not verify their compatibility.
	///
	/// # Errors
	///
	/// Returns an error if resolving the class or method descriptor fails, or JNI cannot create the reflected object.
	pub unsafe fn to_reflected_static_method_unchecked<'other_local>(
		&mut self,
		class: impl Desc<'local, JClass<'other_local>>,
		method_id: impl Desc<'local, JStaticMethodID>,
	) -> Result<JObject<'local>> {
		// SAFETY: The caller guarantees class compatibility and ID validity; JStaticMethodID identifies a static method.
		unsafe { self.to_reflected_method_base(class, method_id, JStaticMethodID::into_raw, true) }
	}

	/// Convert a [`JMethodID`] or [`JStaticMethodID`] into a [`JObject`] with the
	/// corresponding `java.lang.reflect.Method` or `java.lang.reflect.Constructor`
	/// instance.
	///
	/// The `to_jmethodid` function is used to convert the method ID type into
	/// a raw [`sys::jmethodID`].
	///
	/// # Safety
	///
	/// The resolved class must be non-null and compatible with the method ID.
	/// The ID must remain valid and its declaring class must remain loaded until
	/// this call returns. `to_jmethodid` must preserve the resolved method's identity,
	/// and `is_static` must correctly indicate whether it is a static method.
	#[allow(
		clippy::wrong_self_convention,
		reason = "JNI reflection allocates a local reference and therefore requires a mutable Env."
	)]
	pub(super) unsafe fn to_reflected_method_base<'other_local, M, C>(
		&mut self,
		class: C,
		method_id: impl Desc<'local, M>,
		to_jmethodid: impl FnOnce(M) -> crate::sys::jmethodID,
		is_static: bool,
	) -> Result<JObject<'local>>
	where
		M: Copy,
		C: Desc<'local, JClass<'other_local>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class: C::Output = class.lookup(self)?;

		let method_id: crate::sys::jmethodID = to_jmethodid(*method_id.lookup(self)?.as_ref());

		// SAFETY: The caller guarantees class compatibility, ID validity and matching staticness.
		// The resolved class is retained until after the JNI call.
		let reflected: Result<JObject<'local>> = unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_2,
				ToReflectedMethod,
				class.as_ref().as_raw(),
				method_id,
				sys::jboolean::from(is_static)
			)
			.map(|jobject| JObject::from_raw(self, jobject))
		};
		drop(class);
		reflected
	}
}
