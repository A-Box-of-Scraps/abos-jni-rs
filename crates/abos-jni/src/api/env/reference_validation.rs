use super::{Env, Error, IntoAuto, JClass, JObject, JValue, Reference, Result};
use crate::{jni_sig, jni_str, objects::JObjectArray, sys};

impl Env<'_> {
	/// Validate reference arguments against the resolved method's parameter classes.
	///
	/// # Safety
	///
	/// `method` must belong to `class`, and `is_static` must match the method kind.
	/// Each index must identify a reference parameter of that method.
	///
	/// # Errors
	///
	/// Returns `WrongObjectType` for incompatible values, or a JNI/reflection error.
	#[doc(hidden)]
	pub unsafe fn __validate_method_reference_args(
		&self,
		class: &JClass<'_>,
		method: sys::jmethodID,
		is_static: bool,
		args: &[(usize, &JObject<'_>)],
	) -> Result<()> {
		if args.iter().all(|(_, value)| value.is_null()) {
			return Ok(());
		}
		// Reflect the resolved member rather than looking up descriptor names: inherited
		// members and identically named classes from different loaders must keep their identity.
		self.with_local_frame(8, |env| {
			// SAFETY: The caller supplies the matching class, method ID, and static flag.
			let reflected: JObject<'_> = unsafe {
				let raw: sys::jobject = jni_call_post_check_ex_and_null_ret!(
					env,
					v1_2,
					ToReflectedMethod,
					class.as_raw(),
					method,
					sys::jboolean::from(is_static)
				)?;
				JObject::from_raw(env, raw)
			};
			let parameters: JObject<'_> = env
				.call_method(
					&reflected,
					jni_str!("getParameterTypes"),
					jni_sig!("()[Ljava/lang/Class;"),
					&[],
				)?
				.l()?;
			// SAFETY: getParameterTypes returns Class[], for both methods and constructors.
			let parameters: JObjectArray<'_, JClass<'_>> =
				unsafe { JObjectArray::<JClass>::from_raw(env, parameters.into_raw()) };
			for &(index, value) in args.iter().filter(|(_, value)| !value.is_null()) {
				let expected: crate::refs::Auto<'_, JClass<'_>> = parameters.get_element(env, index)?.auto();
				validate_reference(env, value, &expected)?;
			}
			Ok(())
		})
	}

	/// Validate a reference value against the resolved field's class.
	///
	/// # Safety
	///
	/// `field` must identify a reference field of `class`, and `is_static` must match its kind.
	///
	/// # Errors
	///
	/// Returns `WrongObjectType` for incompatible values, or a JNI/reflection error.
	#[doc(hidden)]
	pub unsafe fn __validate_field_reference(
		&self,
		class: &JClass<'_>,
		field: sys::jfieldID,
		is_static: bool,
		value: &JObject<'_>,
	) -> Result<()> {
		if value.is_null() {
			return Ok(());
		}
		self.with_local_frame(8, |env| {
			// SAFETY: The caller supplies the matching class, field ID, and static flag.
			let reflected: JObject<'_> = unsafe {
				let raw: sys::jobject = jni_call_post_check_ex_and_null_ret!(
					env,
					v1_2,
					ToReflectedField,
					class.as_raw(),
					field,
					sys::jboolean::from(is_static)
				)?;
				JObject::from_raw(env, raw)
			};
			let expected: JObject<'_> = env
				.call_method(&reflected, jni_str!("getType"), jni_sig!("()Ljava/lang/Class;"), &[])?
				.l()?;
			// SAFETY: Field.getType returns a Class object.
			let expected: JClass<'_> = unsafe { JClass::from_raw(env, expected.into_raw()) };
			validate_reference(env, value, &expected)
		})
	}
}

pub(super) fn reference_args<'a>(args: &[JValue<'a>]) -> Vec<(usize, &'a JObject<'a>)> {
	args.iter()
		.enumerate()
		.filter_map(|(index, value)| match value {
			JValue::Object(object) => Some((index, *object)),
			_ => None,
		})
		.collect()
}

fn validate_reference(env: &mut Env<'_>, value: &JObject<'_>, expected: &JClass<'_>) -> Result<()> {
	if env.is_instance_of(value, expected)? {
		Ok(())
	} else {
		Err(Error::WrongObjectType)
	}
}
