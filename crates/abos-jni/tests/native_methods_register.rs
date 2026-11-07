#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::errors::Error;
use abos_jni::objects::{JClass, JObject};
use abos_jni::sys::{jint, jlong};
use abos_jni::{Env, NativeMethod, native_method};

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn add_impl<'local>(_env: &mut Env<'local>, _this: JObject<'local>, a: jint, b: jint) -> Result<jint, Error> {
	Ok(a + b)
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn multiply_impl<'local>(
	_env: &mut Env<'local>,
	_this: JObject<'local>,
	a: jint,
	b: jint,
) -> Result<jint, Error> {
	Ok(a * b)
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn static_add_impl<'local>(
	_env: &mut Env<'local>,
	_class: JClass<'local>,
	a: jlong,
	b: jlong,
) -> Result<jlong, Error> {
	Ok(a + b)
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn static_multiply_impl<'local>(
	_env: &mut Env<'local>,
	_class: JClass<'local>,
	a: jlong,
	b: jlong,
) -> Result<jlong, Error> {
	Ok(a * b)
}

const ADD_METHOD: NativeMethod = native_method! {
	name = "staticAdd",
	sig = (a: jlong, b: jlong) -> jlong,
	fn = static_add_impl,
	static = true,
};

#[allow(
	dead_code,
	reason = "Demonstrate manual registration separately from the executable test harness."
)]
fn register_my_methods(env: &mut Env, class: JClass) -> abos_jni::errors::Result<()> {
	const METHODS: &[NativeMethod] = &[
		ADD_METHOD,
		native_method! {
			name = "staticMultiply",
			sig = (a: jlong, b: jlong) -> jlong,
			fn = static_multiply_impl,
			static = true,
		},
		native_method! {
			name = "add",
			sig = (a: jint, b: jint) -> jint,
			fn = add_impl
		},
		native_method! {
			name = "multiply",
			sig = (a: jint, b: jint) -> jint,
			fn = multiply_impl
		},
	];

	// SAFETY: The fixture class declares the names and JNI signatures supplied by these native-method descriptors.
	unsafe {
		env.register_native_methods(class, METHODS)?;
	}

	Ok(())
}

#[test]
fn test_example_compiles() {
	// This test just verifies the example compiles
}
