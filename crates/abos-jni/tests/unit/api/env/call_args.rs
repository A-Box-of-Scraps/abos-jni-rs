#![allow(unsafe_code, reason = "Inspect the active field of converted JNI unions.")]

use super::call_args::{STACK_ARGS, with_jni_args};
use abos_jni::{JValue, sys::jvalue};

#[test]
fn preserves_count_and_order_across_stack_boundary() {
	for count in [0, 1, STACK_ARGS - 1, STACK_ARGS, STACK_ARGS + 1, 32] {
		let args: Vec<JValue<'_>> = (0..count)
			.map(|index| JValue::Long(-i64::try_from(index).expect("small index")))
			.collect();
		let actual: usize = with_jni_args(&args, |converted| {
			assert_eq!(converted.len(), count);
			for (index, value) in converted.iter().enumerate() {
				// SAFETY: Every argument was constructed as a Java long.
				assert_eq!(unsafe { value.j }, -i64::try_from(index).expect("small index"));
			}
			converted.len()
		});
		assert_eq!(actual, count);
	}
}

#[test]
fn propagates_callback_errors() {
	for count in [0, 1, STACK_ARGS + 1] {
		let args: Vec<JValue<'_>> = vec![JValue::Int(1); count];
		let result: Result<(), &str> = with_jni_args(&args, |_values: &[jvalue]| Err("sentinel"));
		assert_eq!(result, Err("sentinel"));
	}
}

#[test]
fn preserves_primitive_and_reference_values_on_both_paths() {
	let null: abos_jni::objects::JObject<'_> = abos_jni::objects::JObject::null();
	let args: [JValue<'_>; 9] = [
		JValue::Bool(1),
		JValue::Byte(-7),
		JValue::Char(0xD800),
		JValue::Short(-1234),
		JValue::Int(-123_456),
		JValue::Long(i64::MIN),
		JValue::Float(-0.0),
		JValue::Double(f64::INFINITY),
		JValue::Object(&null),
	];
	// Every variant is tested with a one-element stack buffer and together on the heap path.
	for argument in &args {
		with_jni_args(std::slice::from_ref(argument), |converted| {
			assert_argument(argument, &converted[0]);
		});
	}
	with_jni_args(&args, |converted| {
		for (argument, value) in args.iter().zip(converted) {
			assert_argument(argument, value);
		}
	});
}

fn assert_argument(argument: &JValue<'_>, value: &jvalue) {
	// SAFETY: Each JValue variant identifies the union field initialized by as_jni.
	unsafe {
		match argument {
			JValue::Bool(expected) => assert_eq!(value.z, *expected),
			JValue::Byte(expected) => assert_eq!(value.b, *expected),
			JValue::Char(expected) => assert_eq!(value.c, *expected),
			JValue::Short(expected) => assert_eq!(value.s, *expected),
			JValue::Int(expected) => assert_eq!(value.i, *expected),
			JValue::Long(expected) => assert_eq!(value.j, *expected),
			JValue::Float(expected) => assert_eq!(value.f.to_bits(), expected.to_bits()),
			JValue::Double(expected) => assert_eq!(value.d.to_bits(), expected.to_bits()),
			JValue::Object(expected) => assert_eq!(value.l, expected.as_raw()),
			JValue::Void => panic!("void is not a method argument"),
		}
	}
}
