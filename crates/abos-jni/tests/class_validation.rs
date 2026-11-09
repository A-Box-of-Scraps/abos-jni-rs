#![allow(
	unsafe_code,
	reason = "Shared test utilities contain JNI FFI helpers; scenarios use safe APIs."
)]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClass, JObject, JString},
};
use assert_matches::assert_matches;
use rusty_fork::rusty_fork_test;

fn assert_usable(env: &mut Env<'_>) -> Result<()> {
	assert!(!env.exception_check());
	let string: JString<'_> = env.new_string("usable")?;
	assert_eq!(string.try_to_string(env)?, "usable");
	Ok(())
}

// Isolate each regression so an invalid JNI call cannot terminate the other tests.
rusty_fork_test! {
#[test]
fn object_arrays_reject_null_element_classes() {
	util::attach_current_thread(|env| {
		let string = env.new_string("initializer")?;
		for length in [0, 1] {
			for initial in [JObject::null(), env.new_local_ref(&string)?.into()] {
				assert_matches!(
					env.new_object_array(length, JClass::null(), initial),
					Err(Error::NullPtr(_))
				);
				assert_usable(env)?;
			}
		}
		Ok(())
	})
	.expect("null element classes return errors without entering JNI allocation");
}

#[test]
fn object_arrays_reject_primitive_and_void_element_classes() {
	util::attach_current_thread(|env| {
		for wrapper in [
			jni_str!("java/lang/Boolean"), jni_str!("java/lang/Byte"),
			jni_str!("java/lang/Character"), jni_str!("java/lang/Short"),
			jni_str!("java/lang/Integer"), jni_str!("java/lang/Long"),
			jni_str!("java/lang/Float"), jni_str!("java/lang/Double"),
			jni_str!("java/lang/Void"),
		] {
			let class = env.get_static_field(wrapper, jni_str!("TYPE"), jni_sig!("Ljava/lang/Class;"))?.l()?;
			let class = env.cast_local::<JClass>(class)?;
			assert!(class.is_primitive(env)?);
			for length in [0, 1] {
				assert_matches!(
					env.new_object_array(length, &class, JObject::null()),
					Err(Error::WrongObjectType)
				);
				assert_usable(env)?;
			}
		}
		Ok(())
	})
	.expect("primitive and void component classes are rejected");
}

#[test]
fn object_arrays_accept_reference_element_classes() {
	util::attach_current_thread(|env| {
		for name in [
			jni_str!("java/lang/Object"), jni_str!("java/lang/String"),
			jni_str!("java/lang/Runnable"), jni_str!("[I"), jni_str!("[Ljava/lang/String;"),
		] {
			let class = env.find_class(name)?;
			assert!(!class.is_primitive(env)?);
			for length in [0, 1] {
				let array = env.new_object_array(length, &class, JObject::null())?;
				assert_eq!(array.len(env)?, usize::try_from(length).expect("nonnegative length"));
			}
		}
		// A primitive array is itself a reference type and can initialize an int[][].
		let ints = env.new_int_array(2)?;
		let matrix = env.new_object_array(1, jni_str!("[I"), &ints)?;
		let element = matrix.get_element(env, 0)?;
		assert!(env.is_same_object(&ints, &element)?);
		assert_usable(env)
	})
	.expect("classes, interfaces, and array classes remain valid component types");
}

#[test]
fn get_superclass_rejects_null_classes() {
	util::attach_current_thread(|env| {
		assert_matches!(env.get_superclass(JClass::null()), Err(Error::NullPtr(_)));
		assert_usable(env)?;
		assert!(env.get_superclass(jni_str!("java/lang/String"))?.is_some());
		assert!(env.get_superclass(jni_str!("java/lang/Object"))?.is_none());
		assert!(env.get_superclass(jni_str!("java/lang/Runnable"))?.is_none());
		Ok(())
	})
	.expect("null class rejection does not change superclass lookup semantics");
}

#[test]
fn unregister_native_methods_rejects_null_classes() {
	util::attach_current_thread(|env| {
		assert_matches!(env.unregister_native_methods(JClass::null()), Err(Error::NullPtr(_)));
		assert_usable(env)?;
		// This class has no native methods, so unregistration has no VM-wide effects.
		env.unregister_native_methods(jni_str!("java/util/ArrayList"))?;
		assert_usable(env)
	})
	.expect("null class rejection leaves native unregistration usable");
}

#[test]
fn element_class_validation_preserves_pending_exceptions() {
	util::attach_current_thread(|env| {
		let class = env.find_class(jni_str!("java/lang/String"))?;
		assert_matches!(env.throw("pending sentinel"), Err(Error::JavaException));
		let original = env.exception_occurred().expect("pending exception");
		assert_matches!(
			env.new_object_array(1, &class, JObject::null()),
			Err(Error::JavaException)
		);
		let remaining = env.exception_occurred().expect("exception remains pending");
		env.exception_clear();
		assert!(env.is_same_object(&original, &remaining)?);
		assert_usable(env)
	})
	.expect("the primitive-class check preserves an already pending exception");
}
}
