use super::*;

impl<'local> Env<'local> {
	/// Convert a [`JMethodID`] into a [`JObject`] with the corresponding
	/// `java.lang.reflect.Method` or `java.lang.reflect.Constructor` instance.
	pub fn to_reflected_method<'other_local>(
		&mut self,
		class: impl Desc<'local, JClass<'other_local>>,
		method_id: impl Desc<'local, JMethodID>,
	) -> Result<JObject<'local>> {
		// Safety: Rust type safety ensures that method_id is a JMethodID, while is_static is false
		unsafe { self.to_reflected_method_base(class, method_id, JMethodID::into_raw, false) }
	}

	/// Convert a [`JStaticMethodID`] into a [`JObject`] with the corresponding
	/// `java.lang.reflect.Method` instance.
	pub fn to_reflected_static_method<'other_local>(
		&mut self,
		class: impl Desc<'local, JClass<'other_local>>,
		method_id: impl Desc<'local, JStaticMethodID>,
	) -> Result<JObject<'local>> {
		// Safety: Rust type safety ensures that method_id is a JStaticMethodID, while is_static is true
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
	/// `is_static` must correctly indicate whether the method ID is for a static method. (The JNI
	/// spec does not define what happens if this is incorrect.)
	#[allow(clippy::wrong_self_convention)]
	pub(super) unsafe fn to_reflected_method_base<'other_local, M>(
		&mut self,
		class: impl Desc<'local, JClass<'other_local>>,
		method_id: impl Desc<'local, M>,
		to_jmethodid: impl FnOnce(M) -> crate::sys::jmethodID,
		is_static: bool,
	) -> Result<JObject<'local>>
	where
		M: Copy,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class = class.lookup(self)?;

		let method_id = to_jmethodid(*method_id.lookup(self)?.as_ref());

		unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_2,
				ToReflectedMethod,
				class.as_ref().as_raw(),
				method_id,
				is_static
			)
			.map(|jobject| JObject::from_raw(self, jobject))
		}
	}
}
