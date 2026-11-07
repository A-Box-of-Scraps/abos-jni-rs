use super::*;

#[test]
pub fn call_method_returning_null() {
	attach_current_thread(|env| {
		let obj: abos_jni::refs::Auto<'_, JObject<'_>> =
			unwrap(env.new_object(EXCEPTION_CLASS, jni_sig!("()V"), &[]), env).auto();
		let message: abos_jni::JValueOwned<'_> = unwrap(
			env.call_method(&obj, jni_str!("getMessage"), jni_sig!("()Ljava/lang/String;"), &[]),
			env,
		);
		let message_ref: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(message.l(), env).auto();

		assert!(message_ref.is_null());
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_instance_of_same_class() {
	attach_current_thread(|env| {
		let obj: abos_jni::refs::Auto<'_, JObject<'_>> =
			unwrap(env.new_object(EXCEPTION_CLASS, jni_sig!("()V"), &[]), env).auto();
		assert!(unwrap(env.is_instance_of(&obj, EXCEPTION_CLASS), env));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_instance_of_superclass() {
	attach_current_thread(|env| {
		let obj: abos_jni::refs::Auto<'_, JObject<'_>> =
			unwrap(env.new_object(ARITHMETIC_EXCEPTION_CLASS, jni_sig!("()V"), &[]), env).auto();
		assert!(unwrap(env.is_instance_of(&obj, EXCEPTION_CLASS), env));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_instance_of_subclass() {
	attach_current_thread(|env| {
		let obj: abos_jni::refs::Auto<'_, JObject<'_>> =
			unwrap(env.new_object(EXCEPTION_CLASS, jni_sig!("()V"), &[]), env).auto();
		assert!(!unwrap(env.is_instance_of(&obj, ARITHMETIC_EXCEPTION_CLASS), env,));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_instance_of_not_superclass() {
	attach_current_thread(|env| {
		let obj: abos_jni::refs::Auto<'_, JObject<'_>> =
			unwrap(env.new_object(ARITHMETIC_EXCEPTION_CLASS, jni_sig!("()V"), &[]), env).auto();
		assert!(!unwrap(env.is_instance_of(&obj, ARRAYLIST_CLASS), env));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_instance_of_null() {
	attach_current_thread(|env| {
		let obj: JObject<'_> = JObject::null();
		assert!(unwrap(env.is_instance_of(&obj, ARRAYLIST_CLASS), env));
		assert!(unwrap(env.is_instance_of(&obj, EXCEPTION_CLASS), env));
		assert!(unwrap(env.is_instance_of(&obj, ARITHMETIC_EXCEPTION_CLASS), env,));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_same_object_diff_references() {
	attach_current_thread(|env| {
		let string: JString<'_> =
			JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");
		let ref_from_string: JString<'_> = unwrap(env.new_local_ref(&string), env);
		assert!(
			env.is_same_object(&string, &ref_from_string)
				.expect("is same object succeeds")
		);
		env.delete_local_ref(ref_from_string);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_same_object_same_reference() {
	attach_current_thread(|env| {
		let string: JString<'_> =
			JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");
		assert!(env.is_same_object(&string, &string).expect("is same object succeeds"));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_not_same_object() {
	attach_current_thread(|env| {
		let string: JString<'_> =
			JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");
		let same_src_str: JString<'_> =
			JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");
		assert!(
			!env.is_same_object(&string, &same_src_str)
				.expect("is same object succeeds")
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn is_not_same_object_null() {
	attach_current_thread(|env| {
		assert!(
			env.is_same_object(JObject::null(), JObject::null())
				.expect("is same object succeeds")
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
