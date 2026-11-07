#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::refs::Auto;
use abos_jni::{
	jni_sig, jni_str,
	objects::{IntoAuto, JList, JString},
	sys::jint,
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn jlist_push_and_iterate() {
	attach_current_thread(|env| {
		let data: &[&abos_jni::strings::JNIStr; 5] = &[
			jni_str!("hello"),
			jni_str!("world"),
			jni_str!("from"),
			jni_str!("jlist"),
			jni_str!("test"),
		];

		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		unwrap(
			data.iter().try_for_each(|s| {
				let string: JString<'_> = JString::from_jni_str(env, s)?;
				let added: bool = list.add(env, &string)?;
				assert!(added);
				Ok(())
			}),
			env,
		);

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, jint::try_from(data.len()).expect("test list length fits jint"));

		let mut collected: Vec<abos_jni::strings::JNIString> = Vec::new();
		unwrap(
			list.iter(env).and_then(|iter| {
				while let Some(obj) = iter.next(env)? {
					let s: JString<'_> = env.cast_local::<JString>(obj)?;
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
fn jlist_get_and_set() {
	attach_current_thread(|env| {
		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		let hello_str: JString<'_> = unwrap(env.new_string("hello"), env);
		let world_str: JString<'_> = unwrap(env.new_string("world"), env);

		unwrap(list.add(env, &hello_str.into()), env);
		unwrap(list.add(env, &world_str.into()), env);

		let first_obj: abos_jni::objects::JObject<'_> = unwrap(list.get(env, 0), env);
		assert!(!first_obj.is_null());
		let first_jstring: JString<'_> = unwrap(env.cast_local::<JString>(first_obj), env);
		let first_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = unwrap(first_jstring.mutf8_chars(env), env);
		assert_eq!(first_str.to_str().as_ref(), "hello");

		let second_obj: abos_jni::objects::JObject<'_> = unwrap(list.get(env, 1), env);
		assert!(!second_obj.is_null());
		let second_jstring: JString<'_> = unwrap(env.cast_local::<JString>(second_obj), env);
		let second_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = unwrap(second_jstring.mutf8_chars(env), env);
		assert_eq!(second_str.to_str().as_ref(), "world");

		let invalid: std::result::Result<abos_jni::objects::JObject<'_>, abos_jni::errors::Error> = list.get(env, 10);
		assert!(invalid.is_err());

		assert!(matches!(
			env.exception_catch().expect_err("invalid test input is rejected"),
			abos_jni::errors::Error::CaughtJavaException {
				ref name, ..
			} if name == "java.lang.IndexOutOfBoundsException"
		));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn jlist_insert_and_remove() {
	attach_current_thread(|env| {
		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		let first_str: JString<'_> = unwrap(env.new_string("first"), env);
		let third_str: JString<'_> = unwrap(env.new_string("third"), env);

		unwrap(list.add(env, &first_str.into()), env);
		unwrap(list.add(env, &third_str.into()), env);

		let second_str: JString<'_> = unwrap(env.new_string("second"), env);
		unwrap(list.insert(env, 1, &second_str), env);

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 3);

		let items: Vec<String> = (0..3)
			.map(|i| {
				let obj: abos_jni::objects::JObject<'_> = unwrap(list.get(env, i), env);
				let jstring: JString<'_> = unwrap(env.cast_local::<JString>(obj), env);
				String::from(unwrap(jstring.mutf8_chars(env), env))
			})
			.collect();

		assert_eq!(items, vec!["first", "second", "third"]);

		let removed_obj: abos_jni::objects::JObject<'_> = unwrap(list.remove(env, 1), env);
		let removed_jstring: JString<'_> = unwrap(env.cast_local::<JString>(removed_obj), env);
		let removed_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> =
			unwrap(removed_jstring.mutf8_chars(env), env);
		assert_eq!(removed_str.to_str().as_ref(), "second");

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 2);

		let first_remaining: abos_jni::objects::JObject<'_> = unwrap(list.get(env, 0), env);
		let first_jstring: JString<'_> = unwrap(env.cast_local::<JString>(first_remaining), env);
		let first_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = unwrap(first_jstring.mutf8_chars(env), env);
		assert_eq!(first_str.to_str().as_ref(), "first");

		let second_remaining: abos_jni::objects::JObject<'_> = unwrap(list.get(env, 1), env);
		let second_jstring: JString<'_> = unwrap(env.cast_local::<JString>(second_remaining), env);
		let second_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = unwrap(second_jstring.mutf8_chars(env), env);
		assert_eq!(second_str.to_str().as_ref(), "third");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn jlist_size_and_remove() {
	attach_current_thread(|env| {
		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 0);

		let first_str: JString<'_> = unwrap(env.new_string("first"), env);
		let second_str: JString<'_> = unwrap(env.new_string("second"), env);
		let third_str: JString<'_> = unwrap(env.new_string("third"), env);

		unwrap(list.add(env, &first_str), env);
		unwrap(list.add(env, &second_str), env);
		unwrap(list.add(env, &third_str), env);

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 3);

		let removed_obj: abos_jni::objects::JObject<'_> = unwrap(list.remove(env, 2), env);
		let removed_jstring: JString<'_> = unwrap(env.cast_local::<JString>(removed_obj), env);
		let removed_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> =
			unwrap(removed_jstring.mutf8_chars(env), env);
		assert_eq!(removed_str.to_str().as_ref(), "third");

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 2);

		let removed_obj: abos_jni::objects::JObject<'_> = unwrap(list.remove(env, 1), env);
		let removed_jstring: JString<'_> = unwrap(env.cast_local::<JString>(removed_obj), env);
		let removed_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> =
			unwrap(removed_jstring.mutf8_chars(env), env);
		assert_eq!(removed_str.to_str().as_ref(), "second");

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 1);

		let removed_obj: abos_jni::objects::JObject<'_> = unwrap(list.remove(env, 0), env);
		let removed_jstring: JString<'_> = unwrap(env.cast_local::<JString>(removed_obj), env);
		let removed_str: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> =
			unwrap(removed_jstring.mutf8_chars(env), env);
		assert_eq!(removed_str.to_str().as_ref(), "first");

		let size: i32 = unwrap(list.size(env), env);
		assert_eq!(size, 0);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn jlist_iterator_empty() {
	attach_current_thread(|env| {
		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		let mut collected: Vec<abos_jni::strings::JNIString> = Vec::new();
		unwrap(
			list.iter(env).and_then(|iter| {
				while let Some(obj) = iter.next(env)? {
					let s: JString<'_> = env.cast_local::<JString>(obj)?;
					let s: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = s.mutf8_chars(env)?;
					collected.push(s.to_owned());
				}
				Ok(())
			}),
			env,
		);

		assert_eq!(collected, [] as [abos_jni::strings::JNIString; 0]);
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn jlist_iterator_with_auto() {
	attach_current_thread(|env| {
		let data: &[&abos_jni::strings::JNIStr; 3] = &[jni_str!("item1"), jni_str!("item2"), jni_str!("item3")];

		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		unwrap(
			data.iter().try_for_each(|s| {
				let string: JString<'_> = JString::from_jni_str(env, s)?;
				let added: bool = list.add(env, &string)?;
				assert!(added);
				Ok(())
			}),
			env,
		);

		let mut collected: Vec<abos_jni::strings::JNIString> = Vec::new();
		unwrap(
			list.iter(env).and_then(|iter| {
				while let Some(obj) = iter.next(env)? {
					let obj: Auto<'_, abos_jni::objects::JObject<'_>> = obj.auto(); // Wrap as Auto<T> to avoid leaking while iterating
					let s: abos_jni::refs::Cast<'_, '_, JString<'_>> = env.as_cast::<JString>(&obj)?;
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
