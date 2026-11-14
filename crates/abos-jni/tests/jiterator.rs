#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	Env,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JIterator, JObject, JThrowable, JValue},
};

mod util;
use util::attach_current_thread;

fn iterator<'local>(env: &mut Env<'local>, has_more: bool) -> Result<JIterator<'local>> {
	static CLASS: std::sync::Once = std::sync::Once::new();
	CLASS.call_once(|| {
		util::load_test_class(env, &util::java_fixtures(), "TestIterator").expect("iterator fixture loads");
	});
	let object: JObject<'_> = env.new_object(
		jni_str!("com/example/TestIterator"),
		jni_sig!("(Z)V"),
		&[JValue::Bool(has_more.into())],
	)?;
	JIterator::cast_local(env, object)
}

#[test]
fn exhausted_iterator_does_not_call_next() {
	attach_current_thread(|env| {
		let iter: JIterator<'_> = iterator(env, false)?;
		for _ in 0..3 {
			assert!(iter.next(env)?.is_none());
			assert!(!env.exception_check());
		}
		assert_eq!(env.get_field(&iter, jni_str!("hasNextCalls"), jni_sig!("I"))?.i()?, 3);
		assert_eq!(env.get_field(&iter, jni_str!("nextCalls"), jni_sig!("I"))?.i()?, 0);
		Ok(())
	})
	.expect("exhaustion is checked before advancing");
}

#[test]
fn has_next_exception_is_preserved_even_if_it_is_no_such_element() {
	attach_current_thread(|env| {
		let iter: JIterator<'_> = iterator(env, true)?;
		let failure: JObject<'_> =
			env.new_object(jni_str!("java/util/NoSuchElementException"), jni_sig!("()V"), &[])?;
		env.set_field(
			&iter,
			jni_str!("hasNextFailure"),
			jni_sig!("Ljava/lang/RuntimeException;"),
			JValue::Object(&failure),
		)?;
		assert!(matches!(iter.next(env), Err(Error::JavaException)));
		let pending: JThrowable<'_> = env.exception_occurred().expect("hasNext failure remains pending");
		env.exception_clear();
		assert!(env.is_same_object(&failure, &pending)?);
		assert_eq!(env.get_field(&iter, jni_str!("nextCalls"), jni_sig!("I"))?.i()?, 0);
		Ok(())
	})
	.expect("hasNext errors are not mistaken for exhaustion");
}

#[test]
fn next_can_still_report_exhaustion_after_has_next() {
	attach_current_thread(|env| {
		let iter: JIterator<'_> = iterator(env, true)?;
		let failure: JObject<'_> =
			env.new_object(jni_str!("java/util/NoSuchElementException"), jni_sig!("()V"), &[])?;
		env.set_field(
			&iter,
			jni_str!("nextFailure"),
			jni_sig!("Ljava/lang/RuntimeException;"),
			JValue::Object(&failure),
		)?;
		assert!(iter.next(env)?.is_none());
		assert!(!env.exception_check());
		assert_eq!(env.get_field(&iter, jni_str!("hasNextCalls"), jni_sig!("I"))?.i()?, 1);
		assert_eq!(env.get_field(&iter, jni_str!("nextCalls"), jni_sig!("I"))?.i()?, 1);
		Ok(())
	})
	.expect("next exhaustion is still caught and cleared");
}

#[test]
fn pending_exception_prevents_even_has_next() {
	attach_current_thread(|env| {
		let iter: JIterator<'_> = iterator(env, false)?;
		assert!(matches!(
			env.throw_new(
				jni_str!("java/util/NoSuchElementException"),
				jni_str!("pending sentinel")
			),
			Err(Error::JavaException)
		));
		let original: JThrowable<'_> = env.exception_occurred().expect("pending exception");
		assert!(matches!(iter.next(env), Err(Error::JavaException)));
		let pending: JThrowable<'_> = env.exception_occurred().expect("exception remains pending");
		env.exception_clear();
		assert!(env.is_same_object(&original, &pending)?);
		assert_eq!(env.get_field(&iter, jni_str!("hasNextCalls"), jni_sig!("I"))?.i()?, 0);
		assert_eq!(env.get_field(&iter, jni_str!("nextCalls"), jni_sig!("I"))?.i()?, 0);
		Ok(())
	})
	.expect("pending exceptions prevent both iterator calls");
}
