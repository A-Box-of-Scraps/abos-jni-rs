#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	errors::Error,
	jni_sig, jni_str,
	objects::{JMap, JMapIter, JObject, JString},
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn jmap_push_and_iterate() {
	attach_current_thread(|env| {
		let data: &[&abos_jni::strings::JNIStr; 4] =
			&[jni_str!("hello"), jni_str!("world"), jni_str!("from"), jni_str!("test")];

		// LinkedHashMap gives predictable iteration order.
		let map_object: JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/LinkedHashMap"), jni_sig!("()V"), &[]),
			env,
		);
		let map: JMap<'_> = unwrap(JMap::cast_local(env, map_object), env);

		unwrap(
			data.iter().try_for_each(|s| {
				JString::from_jni_str(env, s)
					.map(JObject::from)
					.and_then(|s| map.put(env, &s, &s).map(|_| ()))
			}),
			env,
		);

		let mut collected: Vec<abos_jni::strings::JNIString> = Vec::new();
		unwrap(
			map.iter(env).and_then(|mut iter| {
				while let Some(e) = iter.next(env)? {
					let value: JObject<'_> = e.value(env)?;
					let s: JString<'_> = env.cast_local::<JString>(value)?;
					let s: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = s.mutf8_chars(env)?;

					collected.push(s.to_owned());
				}
				Ok(())
			}),
			env,
		);

		let orig: Vec<&abos_jni::strings::JNIStr> = data.to_vec();
		assert_eq!(orig, collected);
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn jmap_iterator_concurrent_modification() {
	attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/util/LinkedHashMap"), jni_sig!("()V"), &[])?;
		let map: JMap<'_> = JMap::cast_local(env, object)?;
		let first: JString<'_> = env.new_string("first")?;
		let second: JString<'_> = env.new_string("second")?;
		map.put(env, &first, &first)?;
		let mut iter: JMapIter<'_> = map.iter(env)?;
		map.put(env, &second, &second)?;

		assert!(matches!(iter.next(env), Err(Error::JavaException)));
		assert!(env.exception_check());
		assert!(matches!(
			env.exception_catch(),
			Err(Error::CaughtJavaException { name, .. }) if name == "java.util.ConcurrentModificationException"
		));
		assert!(!env.exception_check());
		assert_eq!(map.size(env)?, 2);
		Ok(())
	})
	.expect("map iterator failures remain catchable");
}

#[test]
fn jmap_iterator_rejects_non_entry() {
	let fixtures: std::path::PathBuf = util::java_fixtures();
	attach_current_thread(|env| {
		util::load_test_class(env, &fixtures, "TestMalformedMap")?;
		let object: JObject<'_> = env.new_object(jni_str!("com/example/TestMalformedMap"), jni_sig!("()V"), &[])?;
		let map: JMap<'_> = JMap::cast_local(env, object)?;
		let mut iter: JMapIter<'_> = map.iter(env)?;

		assert!(matches!(iter.next(env), Err(Error::WrongObjectType)));
		assert!(!env.exception_check());
		assert!(iter.next(env)?.is_none());
		Ok(())
	})
	.expect("malformed map entries are rejected without a Java exception");
}

#[test]
fn runtime_optional_map_methods_work_when_present() {
	attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/util/HashMap"), jni_sig!("()V"), &[])?;
		let map: JMap<'_> = JMap::cast_local(env, object)?;
		let key: JString<'_> = env.new_string("key")?;
		let first: JString<'_> = env.new_string("first")?;
		let second: JString<'_> = env.new_string("second")?;

		let value: JObject<'_> = map.get_or_default(env, &key, &first)?;
		assert!(env.is_same_object(value, &first)?);
		assert!(map.put_if_absent(env, &key, &first)?.is_null());
		let value: JObject<'_> = map.put_if_absent(env, &key, &second)?;
		assert!(env.is_same_object(value, &first)?);
		let value: JObject<'_> = map.replace(env, &key, &second)?;
		assert!(env.is_same_object(value, &first)?);
		assert!(map.replace_value(env, &key, &second, &first)?);
		let value: JObject<'_> = map.get(env, key.as_ref())?.expect("the key remains present");
		assert!(env.is_same_object(value, &first)?);
		assert_eq!(map.size(env)?, 1);
		Ok(())
	})
	.expect("runtime-optional map methods remain callable when present");
}
