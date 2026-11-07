#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	jni_sig, jni_str,
	objects::{JCollection, JList, JSet},
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn test_generated_alias_methods() {
	use abos_jni::errors::Result;
	attach_current_thread(|env| -> Result<()> {
		// Regression: https://github.com/jni-rs/jni-rs/issues/773
		let null_list: JList<'_> = JList::null();
		let null_collection_ref: &JCollection = null_list.as_ref();
		assert!(null_collection_ref.is_null());

		let list_object: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list: JList<'_> = unwrap(JList::cast_local(env, list_object), env);

		// Also covers: https://github.com/jni-rs/jni-rs/issues/773 bug
		let collection_via_as_ref: &JCollection = list.as_ref();
		assert_eq!(collection_via_as_ref.as_raw(), list.as_raw());
		let _size0: i32 = unwrap(collection_via_as_ref.size(env), env);

		let collection_via_method: abos_jni::refs::Cast<'_, '_, JCollection<'_>> = list.as_collection();
		assert_eq!(collection_via_method.as_raw(), list.as_raw());
		let _size1: i32 = unwrap(collection_via_method.size(env), env);

		let list_object2: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[]),
			env,
		);
		let list2: JList<'_> = unwrap(JList::cast_local(env, list_object2), env);
		let collection_via_from: JCollection = list2.into();
		let _size2: i32 = unwrap(collection_via_from.size(env), env);

		let set_object: abos_jni::objects::JObject<'_> =
			unwrap(env.new_object(jni_str!("java/util/HashSet"), jni_sig!("()V"), &[]), env);
		let set: JSet<'_> = unwrap(JSet::cast_local(env, set_object), env);
		let set_collection_via_method: abos_jni::refs::Cast<'_, '_, JCollection<'_>> = set.as_collection();
		let _size3: i32 = unwrap(set_collection_via_method.size(env), env);

		let set_object2: abos_jni::objects::JObject<'_> =
			unwrap(env.new_object(jni_str!("java/util/HashSet"), jni_sig!("()V"), &[]), env);
		let set2: JSet<'_> = unwrap(JSet::cast_local(env, set_object2), env);
		let set_collection_via_from: JCollection = set2.into();
		let _size4: i32 = unwrap(set_collection_via_from.size(env), env);

		Ok(())
	})
	.expect("generated aliases preserve the Java object and allow method calls");
}
