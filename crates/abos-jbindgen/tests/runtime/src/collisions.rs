//! Exercise renamed members rather than only checking that generated code compiles.
use crate::{bindings::com::example::MemberCollisions, collision_overrides::com::example::MemberCollisions as Renamed};

pub fn check(env: &mut abos_jni::Env<'_>) -> abos_jni::errors::Result<()> {
	let text = abos_jni::objects::JString::from_str(env, "four")?;
	let instance = MemberCollisions::new_int(env, 42)?;
	assert_eq!(instance.new_int_(env)?, 42);
	assert_eq!(instance.new_int__(env)?, 142);
	instance.set_new_int_(env, 7)?;
	assert_eq!(instance.new_int_(env)?, 7);
	assert_eq!(instance.set_new_int__(env, 3)?, 203);
	instance.set___(env, 9)?;
	assert_eq!(instance.__(env)?, 9);
	assert_eq!(instance.set____(env)?, 309);
	let from_string = MemberCollisions::new_string(env, &text)?;
	assert_eq!(from_string.new_int_(env)?, 4);

	let renamed = Renamed::make(env, 5)?;
	let other = Renamed::make_(env, &text)?;
	assert_eq!(renamed.new_int(env)?, 5);
	assert_eq!(other.new_int(env)?, 4);
	renamed.set_make__(env, 17)?;
	assert_eq!(renamed.make__(env)?, 17);
	assert_eq!(renamed.pick_name(env, 2)?, 12);
	assert_eq!(renamed.pick_name_(env, &text)?, 24);
	let mixed = crate::native_collisions::com::example::MixedMethods::new(env)?;
	assert_eq!(mixed.call(env, 5)?, 6);
	assert_eq!(mixed.call_(env, 123)?, 42);
	Ok(())
}

impl crate::native_collisions::com::example::MixedMethodsNativeInterface
	for crate::native_collisions::com::example::MixedMethodsAPI
{
	type Error = abos_jni::errors::Error;

	fn call_<'local>(
		_env: &mut abos_jni::Env<'local>,
		_this: crate::native_collisions::com::example::MixedMethods<'local>,
		_value: abos_jni::sys::jlong,
	) -> Result<abos_jni::sys::jint, Self::Error> {
		Ok(42)
	}
}
