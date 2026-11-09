#![allow(unsafe_code, reason = "Tests validate raw JNI values and exported native ABIs.")]
#![allow(missing_docs, reason = "Integration test fixtures.")]

use abos_jni::objects::{JValue, JValueOwned};
use abos_jni::sys::{JNI_FALSE, JNI_TRUE};

#[test]
fn jvalue_converts_between_rust_bool_and_raw_bytes() {
	for (value, raw) in [(false, JNI_FALSE), (true, JNI_TRUE)] {
		let borrowed: JValue<'_> = JValue::from(value);
		let owned: JValueOwned<'_> = JValueOwned::from(value);
		assert_eq!(bool::try_from(borrowed).expect("boolean conversion succeeds"), value);
		assert_eq!(bool::try_from(owned).expect("boolean conversion succeeds"), value);
		// SAFETY: as_jni initializes the z member for a Bool variant.
		assert_eq!(unsafe { borrowed.as_jni().z }, raw);
	}
	for raw in 0..=u8::MAX {
		let value: JValue<'_> = JValue::from(raw);
		assert_eq!(u8::try_from(value).expect("boolean conversion succeeds"), raw);
		assert_eq!(value.z().expect("boolean conversion succeeds"), raw != JNI_FALSE);
		assert_eq!(
			JValueOwned::from(raw).z().expect("boolean conversion succeeds"),
			raw != JNI_FALSE
		);
		// SAFETY: The Bool variant initializes z, including noncanonical raw bytes.
		assert_eq!(unsafe { value.as_jni().z }, raw);
	}
}

#[cfg(feature = "invocation")]
mod util;

#[cfg(feature = "invocation")]
mod jvm {
	use super::util;
	use abos_jni::errors::Result;
	use abos_jni::objects::JClass;
	use abos_jni::sys::jboolean;
	use abos_jni::{Env, EnvUnowned, bind_java_type, jni_str, native_method};

	bind_java_type! {
		BooleanBindings => "com.example.TestBooleanBindings",
		constructors { fn new() },
		fields {
			value: bool,
			static static_value: bool,
			raw_value { name = "value", sig = jboolean },
		},
		methods {
			fn echo(input: bool) -> bool,
			fn echo_raw { name = "echo", sig = (input: jboolean) -> jboolean },
			static fn invert_standalone(input: bool) -> bool,
		},
		native_methods {
			fn invert_bound {
				sig = (input: bool) -> bool,
				static = true,
				export = true,
			},
		}
	}

	impl BooleanBindingsNativeInterface for BooleanBindingsAPI {
		type Error = abos_jni::errors::Error;
		fn invert_bound<'local>(_env: &mut Env<'local>, _class: JClass<'local>, input: bool) -> Result<bool> {
			Ok(!input)
		}
	}

	#[expect(clippy::unnecessary_wraps, reason = "Native callbacks return Result.")]
	const fn invert_standalone<'local>(_env: &mut Env<'local>, _class: JClass<'local>, input: bool) -> Result<bool> {
		Ok(!input)
	}

	unsafe extern "system" {
		fn Java_com_example_TestBooleanBindings_invertStandalone__Z<'local>(
			env: EnvUnowned<'local>,
			class: JClass<'local>,
			input: jboolean,
		) -> jboolean;
	}

	#[test]
	fn generated_bindings_convert_booleans_at_both_boundaries() {
		let fixtures: std::path::PathBuf = util::java_fixtures();
		util::attach_current_thread(|env| {
			util::load_test_class(env, &fixtures, "TestBooleanBindings")?;
			let object: BooleanBindings<'_> = BooleanBindings::new(env)?;
			let class: JClass<'_> = env.find_class(jni_str!("com/example/TestBooleanBindings"))?;
			// SAFETY: The generated native wrapper matches the fixture's static (Z)Z method.
			unsafe {
				env.register_native_methods(
					&class,
					&[native_method! {
						static fn invert_standalone(input: bool) -> bool,
						fn = invert_standalone,
						export = true,
						java_type = "com.example.TestBooleanBindings",
					}],
				)?;
			}
			for value in [false, true] {
				object.set_value(env, value)?;
				assert_eq!(object.value(env)?, value);
				assert_eq!(object.raw_value(env)?, jboolean::from(value));
				BooleanBindings::set_static_value(env, value)?;
				assert_eq!(BooleanBindings::static_value(env)?, value);
				assert_eq!(object.echo(env, value)?, value);
				assert_eq!(object.echo_raw(env, jboolean::from(value))?, jboolean::from(value));
				assert_eq!(BooleanBindings::invert_standalone(env, value)?, !value);
			}
			// Call the exported functions using the raw unsigned-byte ABI. The wrapper
			// must normalize incoming bytes before passing them to Rust bool callbacks.
			for raw in [0, 1, 2, 255] {
				// SAFETY: Each Env and class is live, and both exports use the (Z)Z ABI.
				unsafe {
					let class: JClass<'_> = env.find_class(jni_str!("com/example/TestBooleanBindings"))?;
					let result: jboolean = Java_com_example_TestBooleanBindings_invertBound__Z(
						EnvUnowned::from_raw(env.get_raw()),
						class,
						raw,
					);
					assert_eq!(result, jboolean::from(raw == 0));
					let class: JClass<'_> = env.find_class(jni_str!("com/example/TestBooleanBindings"))?;
					let result: jboolean = Java_com_example_TestBooleanBindings_invertStandalone__Z(
						EnvUnowned::from_raw(env.get_raw()),
						class,
						raw,
					);
					assert_eq!(result, jboolean::from(raw == 0));
				}
			}
			Ok(())
		})
		.expect("boolean binding roundtrip succeeds");
	}
}
