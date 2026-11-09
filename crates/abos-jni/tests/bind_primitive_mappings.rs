#![allow(unsafe_code, reason = "Binding macros generate JNI calls.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{bind_java_type, sys::jlong};

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq)]
pub struct Handle(jlong);

// Deliberately change the representation so raw casts or transmutation cannot pass the tests.
impl From<Handle> for jlong {
	fn from(value: Handle) -> Self {
		value.0.wrapping_add(1)
	}
}

impl From<jlong> for Handle {
	fn from(value: jlong) -> Self {
		Self(value.wrapping_sub(1))
	}
}

bind_java_type! {
	PrimitiveMappings => "com.example.TestPrimitiveMappings",
	type_map = { unsafe Handle => long },
	constructors { fn new(value: Handle) },
	methods {
		fn echo(value: Handle) -> Handle,
		static fn echo_static(value: Handle) -> Handle,
		fn echo_raw { name = "echo", sig = (value: Handle) -> jlong },
		fn echo_from_raw { name = "echo", sig = (value: jlong) -> Handle },
	},
	fields {
		value: Handle,
		static static_value: Handle,
		raw_value { name = "value", sig = jlong },
		static raw_static_value { name = "staticValue", sig = jlong },
	},
}

#[test]
fn converts_custom_primitives_in_calls_and_field_access() {
	let fixtures: std::path::PathBuf = util::java_fixtures();
	util::attach_current_thread(|env| {
		util::load_test_class(env, &fixtures, "TestPrimitiveMappings")?;
		for value in [jlong::MIN, -1, 0, 42, jlong::MAX] {
			let object: PrimitiveMappings<'_> = PrimitiveMappings::new(env, Handle(value))?;
			assert_eq!(object.value(env)?, Handle(value));
			assert_eq!(object.raw_value(env)?, value.wrapping_add(1));
			assert_eq!(object.echo(env, Handle(value))?, Handle(value));
			assert_eq!(object.echo_raw(env, Handle(value))?, value.wrapping_add(1));
			assert_eq!(object.echo_from_raw(env, value)?, Handle(value.wrapping_sub(1)));
			assert_eq!(PrimitiveMappings::echo_static(env, Handle(value))?, Handle(value));

			let updated: jlong = value.wrapping_add(17);
			object.set_value(env, Handle(updated))?;
			assert_eq!(object.value(env)?, Handle(updated));
			assert_eq!(object.raw_value(env)?, updated.wrapping_add(1));
			PrimitiveMappings::set_static_value(env, Handle(updated))?;
			assert_eq!(PrimitiveMappings::static_value(env)?, Handle(updated));
			assert_eq!(PrimitiveMappings::raw_static_value(env)?, updated.wrapping_add(1));
		}
		Ok(())
	})
	.expect("custom primitive conversions work in both directions");
}
