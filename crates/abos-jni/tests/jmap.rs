#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	jni_sig, jni_str,
	objects::{JMap, JObject, JString},
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn jmap_push_and_iterate() {
	attach_current_thread(|env| {
		let data: &[&abos_jni::strings::JNIStr; 4] =
			&[jni_str!("hello"), jni_str!("world"), jni_str!("from"), jni_str!("test")];

		// Create a new map. Use LinkedHashMap to have predictable iteration order
		let map_object: JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/LinkedHashMap"), jni_sig!("()V"), &[]),
			env,
		);
		let map: JMap<'_> = unwrap(JMap::cast_local(env, map_object), env);

		// Push all strings
		unwrap(
			data.iter().try_for_each(|s| {
				JString::from_jni_str(env, s)
					.map(JObject::from)
					.and_then(|s| map.put(env, &s, &s).map(|_| ()))
			}),
			env,
		);

		// Collect the keys using the JMap iterator
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
