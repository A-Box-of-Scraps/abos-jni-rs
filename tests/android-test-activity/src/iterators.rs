use abos_jni::{
	Env, jni_sig, jni_str,
	objects::{JMap, JMapIter, JObject, JString},
};

use crate::test_error::{TestError, TestResult, require};

pub fn test_array_map(env: &mut Env<'_>) -> TestResult {
	env.with_local_frame(32, |env| {
		let object = env.new_object(jni_str!("android/util/ArrayMap"), jni_sig!("()V"), &[])?;
		let map = JMap::cast_local(env, object)?;
		let mut empty = map.iter(env)?;
		for _ in 0..3 {
			require(empty.next(env)?.is_none(), "empty ArrayMap returned an entry")?;
			require(!env.exception_check(), "empty ArrayMap left a pending exception")?;
		}

		let key = JString::from_str(env, "key")?;
		let value = JString::from_str(env, "value")?;
		let null_key = JString::from_str(env, "null-value")?;
		map.put(env, &key, &value)?;
		map.put(env, &null_key, &JObject::null())?;
		let mut iter: JMapIter<'_> = map.iter(env)?;
		let mut entries = Vec::new();
		// Bound the traversal so a broken exhausted iterator fails rather than looping forever.
		for _ in 0..2 {
			let entry = iter
				.next(env)?
				.ok_or_else(|| TestError::Assertion("ArrayMap iteration ended early".to_owned()))?;
			// ArrayMap may reuse its entry object; inspect it before advancing again.
			let key = entry.key(env)?;
			let key = JString::cast_local(env, key)?.try_to_string(env)?;
			let value = entry.value(env)?;
			let value = if value.is_null() {
				None
			} else {
				Some(JString::cast_local(env, value)?.try_to_string(env)?)
			};
			entries.push((key, value));
		}
		entries.sort();
		require(
			entries
				== [
					("key".to_owned(), Some("value".to_owned())),
					("null-value".to_owned(), None),
				],
			"ArrayMap returned incorrect entries",
		)?;
		for _ in 0..3 {
			require(iter.next(env)?.is_none(), "exhausted ArrayMap returned an entry")?;
			require(!env.exception_check(), "exhausted ArrayMap left a pending exception")?;
		}
		Ok("ArrayMap empty, populated, null-value, and repeated exhaustion checks passed".to_owned())
	})
}
