use super::{Error, JObject, JValue, MethodSignature, validate_call_args};
use crate::jni_sig;

#[test]
fn validates_argument_count_and_base_types() {
	let sig: MethodSignature<'_, '_> = jni_sig!("(ILjava/lang/Object;[I)V");
	let null: JObject<'_> = JObject::null();
	let valid: [JValue<'_>; 3] = [JValue::Int(1), JValue::Object(&null), JValue::Object(&null)];
	assert!(validate_call_args(&sig, &valid).is_ok());
	assert!(matches!(
		validate_call_args(&sig, &valid[..2]),
		Err(Error::InvalidArgList(_))
	));
	let extra: [JValue<'_>; 4] = [valid[0], valid[1], valid[2], JValue::Int(2)];
	assert!(matches!(
		validate_call_args(&sig, &extra),
		Err(Error::InvalidArgList(_))
	));
	for (index, value) in [
		(0, JValue::Long(1)),
		(0, JValue::Object(&null)),
		(1, JValue::Int(1)),
		(2, JValue::Int(1)),
	] {
		let mut args: [JValue<'_>; 3] = valid;
		args[index] = value;
		assert!(
			matches!(validate_call_args(&sig, &args), Err(Error::InvalidArgList(_))),
			"argument {index}"
		);
	}
}

#[test]
fn validates_empty_argument_list() {
	let sig: MethodSignature<'_, '_> = jni_sig!("()V");
	assert!(validate_call_args(&sig, &[]).is_ok());
	assert!(matches!(
		validate_call_args(&sig, &[JValue::Int(1)]),
		Err(Error::InvalidArgList(_))
	));
}
