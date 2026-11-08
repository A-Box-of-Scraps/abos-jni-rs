use crate::sys;

use super::{Desc, Env, JClass, JMethodID, JObject, JStaticMethodID, Reference, Result};

impl<'local> Env<'local> {
	/// Convert a [`JMethodID`] into a [`JObject`] with the corresponding
	/// `java.lang.reflect.Method` or `java.lang.reflect.Constructor` instance.
	///
	/// # Errors
	///
	/// Returns an error if resolving the class or method descriptor fails, or JNI cannot create the reflected object.
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
	///
	/// # Errors
	///
	/// Returns an error if resolving the class or method descriptor fails, or JNI cannot create the reflected object.
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

		// SAFETY: The caller guarantees is_static matches the resolved method ID; the class remains live during reflection.
		unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_2,
				ToReflectedMethod,
				class.as_ref().as_raw(),
				method_id,
				sys::jboolean::from(is_static)
			)
			.map(|jobject| JObject::from_raw(self, jobject))
		}
	}
}
