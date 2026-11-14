#![allow(unsafe_code, reason = "Shared JNI test utilities contain raw FFI operations.")]
#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	Env, JValue,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClass, JObject, JString, JThrowable},
	signature::RuntimeMethodSignature,
};

mod util;

#[test]
fn dynamic_argument_storage_preserves_calls_and_validation() {
	util::attach_current_thread(|env| {
		util::load_test_class(env, &util::java_fixtures(), "DynamicCallArguments")?;
		let class: JClass<'_> = env.find_class(jni_str!("com/example/DynamicCallArguments"))?;
		for count in [0, 8, 9] {
			check_primitive_calls(env, &class, count)?;
		}
		for count in [2, 8, 9, 16] {
			check_reference_calls(env, &class, count)?;
		}
		Ok(())
	})
	.expect("stack and heap argument paths keep validation and invocation behavior");
}

fn check_primitive_calls(env: &mut Env<'_>, class: &JClass<'_>, count: usize) -> Result<()> {
	let parameters: String = "I".repeat(count);
	let signature: RuntimeMethodSignature = RuntimeMethodSignature::from_str(format!("({parameters})I"))?;
	let constructor: RuntimeMethodSignature = RuntimeMethodSignature::from_str(format!("({parameters})V"))?;
	let mut args: Vec<JValue<'_>> = (0..count)
		.map(|index| JValue::Int(i32::try_from(index + 1).expect("small index")))
		.collect();
	let expected: i32 = i32::try_from(count * (count + 1) / 2).expect("small sum");
	let object: JObject<'_> = env.new_object(class, constructor.method_signature(), &args)?;
	assert_eq!(env.get_field(&object, jni_str!("total"), jni_sig!("I"))?.i()?, expected);
	assert_eq!(
		env.call_method(&object, jni_str!("sum"), signature.method_signature(), &args)?
			.i()?,
		expected
	);
	assert_eq!(
		env.call_nonvirtual_method(&object, class, jni_str!("sum"), signature.method_signature(), &args)?
			.i()?,
		expected
	);
	assert_eq!(
		env.call_static_method(class, jni_str!("sumStatic"), signature.method_signature(), &args)?
			.i()?,
		expected
	);

	let before: i32 = calls(env, class)?;
	args.push(JValue::Int(1));
	assert_invalid_args(env.new_object(class, constructor.method_signature(), &args));
	assert_invalid_args(env.call_method(&object, jni_str!("sum"), signature.method_signature(), &args));
	assert_invalid_args(env.call_nonvirtual_method(
		&object,
		class,
		jni_str!("sum"),
		signature.method_signature(),
		&args,
	));
	assert_invalid_args(env.call_static_method(class, jni_str!("sumStatic"), signature.method_signature(), &args));
	args.pop();
	if let Some(first) = args.first_mut() {
		*first = JValue::Long(1);
		assert_invalid_args(env.new_object(class, constructor.method_signature(), &args));
		assert_invalid_args(env.call_method(&object, jni_str!("sum"), signature.method_signature(), &args));
		assert_invalid_args(env.call_nonvirtual_method(
			&object,
			class,
			jni_str!("sum"),
			signature.method_signature(),
			&args,
		));
		assert_invalid_args(env.call_static_method(class, jni_str!("sumStatic"), signature.method_signature(), &args));
	}
	assert_eq!(calls(env, class)?, before, "invalid calls must not reach Java bodies");
	assert!(!env.exception_check());
	Ok(())
}

fn check_reference_calls(env: &mut Env<'_>, class: &JClass<'_>, count: usize) -> Result<()> {
	let parameters: String = (0..count)
		.map(|index| if index % 2 == 0 { "I" } else { "Ljava/lang/String;" })
		.collect();
	let signature: RuntimeMethodSignature = RuntimeMethodSignature::from_str(format!("({parameters})I"))?;
	let string: JString<'_> = env.new_string("value")?;
	let null: JObject<'_> = JObject::null();
	for value in [&null, string.as_ref()] {
		let args: Vec<JValue<'_>> = (0..count)
			.map(|index| {
				if index % 2 == 0 {
					JValue::Int(1)
				} else {
					JValue::Object(value)
				}
			})
			.collect();
		let expected: usize = count.div_ceil(2) + if value.is_null() { 0 } else { 5 * (count / 2) };
		assert_eq!(
			env.call_static_method(class, jni_str!("mixed"), signature.method_signature(), &args)?
				.i()?,
			i32::try_from(expected).expect("small sum")
		);
	}
	let incompatible: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
	let mut args: Vec<JValue<'_>> = (0..count)
		.map(|index| {
			if index % 2 == 0 {
				JValue::Int(1)
			} else {
				JValue::Object(&string)
			}
		})
		.collect();
	let before: i32 = calls(env, class)?;
	let last_reference: usize = (count / 2) * 2 - 1;
	args[last_reference] = JValue::Object(&incompatible);
	assert!(matches!(
		env.call_static_method(class, jni_str!("mixed"), signature.method_signature(), &args),
		Err(Error::WrongObjectType)
	));
	assert!(!env.exception_check());
	assert_eq!(calls(env, class)?, before);

	args[last_reference] = JValue::Object(&string);
	assert!(matches!(
		env.throw_new(
			jni_str!("java/lang/IllegalStateException"),
			jni_str!("pending sentinel")
		),
		Err(Error::JavaException)
	));
	let original: JThrowable<'_> = env.exception_occurred().expect("pending exception");
	assert!(matches!(
		env.call_static_method(class, jni_str!("mixed"), signature.method_signature(), &args),
		Err(Error::JavaException)
	));
	let remaining: JThrowable<'_> = env.exception_occurred().expect("exception remains pending");
	env.exception_clear();
	assert!(env.is_same_object(original, remaining)?);
	assert_eq!(calls(env, class)?, before);
	Ok(())
}

fn calls(env: &mut Env<'_>, class: &JClass<'_>) -> Result<i32> {
	env.get_static_field(class, jni_str!("calls"), jni_sig!("I"))?.i()
}

fn assert_invalid_args<T: std::fmt::Debug>(result: Result<T>) {
	assert!(matches!(
		result.expect_err("invalid arguments must be rejected"),
		Error::InvalidArgList(_)
	));
}
