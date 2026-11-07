//! Renamed Java members must coexist with the wrapper's reference API.
use crate::generated_members::com::example::GeneratedMembers;
use abos_jni::{Env, errors::Result, objects::JObject, refs::Reference};

pub fn check(env: &mut Env<'_>) -> Result<()> {
	let instance = GeneratedMembers::null_(env)?;
	assert!(!instance.is_null());
	assert!(GeneratedMembers::null().is_null());
	assert_eq!(instance.from_raw_(env)?, 10);
	instance.set_from_raw_(env, 20)?;
	assert_eq!(instance.from_raw__(env)?, 21);
	assert_eq!(instance.into_raw_(env)?, 12);
	assert_eq!(instance.into_raw_int(env, 5)?, 17);
	assert_eq!(instance.from_raw___(env)?, 17);
	assert_eq!(GeneratedMembers::cast_local_(env)?, 13);
	assert_eq!(instance.as_raw_(env)?, 14);
	assert_eq!(instance.as_ref_(env)?, 15);
	assert_eq!(instance.is_null_(env)?, 16);
	let reference: &JObject<'_> = instance.as_ref();
	assert_eq!(instance.as_raw(), reference.as_raw());
	let owned = env.new_local_ref(reference)?;
	let cast = GeneratedMembers::cast_local(env, owned)?;
	assert!(env.is_same_object(&instance, &cast)?);
	let from_source = crate::bindings::com::example::GeneratedMembers::new(env)?;
	assert_eq!(from_source.from_raw__(env)?, 11);
	assert_eq!(from_source.into_raw_(env)?, 12);
	// Names are available when the corresponding helper is disabled.
	let no_init = crate::helper_no_init::com::example::jni_init::new(env)?;
	no_init.set_value(env, 42)?;
	assert_eq!(no_init.get_value(env)?, 42);
	let no_interface = crate::bindings::com::example::GeneratedNativesNativeInterface::new(env)?;
	assert!(!no_interface.is_null());
	let native = crate::generated_natives::com::example::GeneratedNatives::new(env)?;
	assert_eq!(native.invoke(env)?, 42);
	Ok(())
}

impl crate::generated_natives::com::example::GeneratedNativesNativeInterface
	for crate::generated_natives::com::example::GeneratedNativesAPI
{
	type Error = abos_jni::errors::Error;

	fn from_raw_<'local>(
		_env: &mut Env<'local>,
		_this: crate::generated_natives::com::example::GeneratedNatives<'local>,
	) -> Result<i32> {
		Ok(42)
	}
}
