use std::sync::OnceLock;

use super::{Env, Error, Global, IntoAuto, JClass, JObject, JValue, Reference, Result};
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
			let parameters: JObjectArray<'_, JClass<'_>> =
				unsafe { method_parameter_types(env, class, method, is_static)? };
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

fn validate_reference(env: &Env<'_>, value: &JObject<'_>, expected: &JClass<'_>) -> Result<()> {
	if env.is_instance_of_class(value, expected)? {
		Ok(())
	} else {
		Err(Error::WrongObjectType)
	}
}

/// Parameter classes cached by a generated binding whose API retains the method's class.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct MethodReferenceCache {
	parameters: OnceLock<Vec<Global<JClass<'static>>>>,
}

impl MethodReferenceCache {
	/// Creates an empty parameter-class cache for one generated method or constructor.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			parameters: OnceLock::new(),
		}
	}

	/// Checks references against parameter classes cached on first successful non-null use.
	///
	/// # Safety
	///
	/// Every call must use the same resolved method, static flag and ordered parameter indices.
	/// The non-null `class` must match `method` and remain loaded for the cache's lifetime.
	/// Each index in `args` must identify a reference parameter of that method.
	///
	/// # Errors
	///
	/// Returns `WrongObjectType` for incompatible values, or a JNI/reflection error.
	pub unsafe fn validate(
		&self,
		env: &Env<'_>,
		class: &JClass<'_>,
		method: sys::jmethodID,
		is_static: bool,
		args: &[(usize, &JObject<'_>)],
	) -> Result<()> {
		if args.iter().all(|(_, value)| value.is_null()) {
			return Ok(());
		}
		let parameters: &[Global<JClass<'static>>] = self.get_or_try_init(|| {
			// SAFETY: The caller retains the class and supplies its matching method and parameter indices.
			unsafe { cache_method_parameter_types(env, class, method, is_static, args) }
		})?;
		for (expected, &(_, value)) in parameters.iter().zip(args) {
			if !value.is_null() {
				validate_reference(env, value, expected)?;
			}
		}
		Ok(())
	}

	fn get_or_try_init(
		&self,
		initialize: impl FnOnce() -> Result<Vec<Global<JClass<'static>>>>,
	) -> Result<&[Global<JClass<'static>>]> {
		if let Some(parameters) = self.parameters.get() {
			return Ok(parameters);
		}
		// Reflection can load classes and re-enter Rust. Do not hold a OnceLock
		// initialization lock across JVM calls; racing/reentrant initializers may repeat work.
		let parameters: Vec<Global<JClass<'static>>> = initialize()?;
		drop(self.parameters.set(parameters));
		Ok(self.parameters.get().expect("parameter cache was initialized"))
	}
}

unsafe fn cache_method_parameter_types(
	env: &Env<'_>,
	class: &JClass<'_>,
	method: sys::jmethodID,
	is_static: bool,
	args: &[(usize, &JObject<'_>)],
) -> Result<Vec<Global<JClass<'static>>>> {
	env.with_local_frame(8, |env| {
		// SAFETY: The caller supplies a live class and its matching method and kind.
		let parameters: JObjectArray<'_, JClass<'_>> =
			unsafe { method_parameter_types(env, class, method, is_static)? };
		args.iter()
			.map(|&(index, _)| {
				// Cache every reference parameter, including currently-null arguments.
				let parameter: crate::refs::Auto<'_, JClass<'_>> = parameters.get_element(env, index)?.auto();
				env.new_global_ref(&*parameter)
			})
			.collect()
	})
}

// Reflect the resolved member, not names from its descriptor: inherited methods and
// same-named classes from different loaders must retain their actual parameter identities.
unsafe fn method_parameter_types<'local>(
	env: &mut Env<'local>,
	class: &JClass<'_>,
	method: sys::jmethodID,
	is_static: bool,
) -> Result<JObjectArray<'local, JClass<'local>>> {
	// SAFETY: The caller supplies a live class and its matching method ID and static flag.
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
	// SAFETY: Method.getParameterTypes and Constructor.getParameterTypes both return Class[].
	Ok(unsafe { JObjectArray::<JClass>::from_raw(env, parameters.into_raw()) })
}

#[cfg(test)]
#[path = "../../../tests/unit/api/env/reference_validation.rs"]
mod tests;
