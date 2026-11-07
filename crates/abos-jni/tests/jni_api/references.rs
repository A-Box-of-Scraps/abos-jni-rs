use super::*;

#[test]
fn get_super_class_ok() {
	attach_current_thread(|env| {
		let result: std::result::Result<Option<abos_jni::objects::JClass<'_>>, abos_jni::errors::Error> =
			env.get_superclass(ARRAYLIST_CLASS);
		assert!(result.is_ok());
		assert!(result.expect("test operation succeeds").is_some());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn get_super_class_null() {
	attach_current_thread(|env| {
		let result: std::result::Result<Option<abos_jni::objects::JClass<'_>>, abos_jni::errors::Error> =
			env.get_superclass(jni_str!("java/lang/Object"));
		assert!(result.is_ok());
		assert!(result.expect("test operation succeeds").is_none());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn local_ref_null() {
	attach_current_thread(|env| {
		let null_obj: JObject<'_> = JObject::null();

		let result: std::result::Result<JObject<'_>, abos_jni::errors::Error> =
			env.new_local_ref::<&JObject>(&null_obj);
		assert!(result.is_ok());
		assert!(result.expect("test operation succeeds").is_null());

		// "delete" null reference
		env.delete_local_ref(null_obj);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn new_global_ref_null() {
	attach_current_thread(|env| {
		let null_obj: JObject<'_> = JObject::null();
		let result: std::result::Result<abos_jni::refs::Global<JObject<'_>>, abos_jni::errors::Error> =
			env.new_global_ref(null_obj);
		assert!(result.is_ok());
		assert!(result.expect("test operation succeeds").is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn new_weak_ref_null() {
	attach_current_thread(|env| {
		let null_obj: JObject<'_> = JObject::null();
		let result: std::result::Result<Weak<JObject<'_>>, abos_jni::errors::Error> = env.new_weak_ref(null_obj);
		assert!(matches!(result, Err(Error::ObjectFreed)));

		let null_weak: Weak<JObject<'static>> = Weak::null();
		assert!(
			null_weak
				.is_garbage_collected(env)
				.expect("is garbage collected succeeds")
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn auto_null() {
	let null_obj: JObject<'_> = JObject::null();
	{
		let auto_ref: abos_jni::refs::Auto<'_, JObject<'_>> = null_obj.auto();
		assert!(auto_ref.is_null());
	}
}

#[test]
fn test_call_nonvirtual_method() {
	attach_current_thread(|env| {
		let a_string: JObject<'_> = JObject::from(env.new_string("test").expect("test string can be allocated"));
		let another_string: JObject<'_> = JObject::from(env.new_string("test").expect("test string can be allocated"));

		// The `equals` method in java/lang/Object will compare the reference
		let obj_class: abos_jni::objects::JClass<'_> = env
			.find_class(jni_str!("java/lang/Object"))
			.expect("test class can be resolved");
		let object_result: bool = env
			.call_nonvirtual_method(
				&a_string,
				&obj_class,
				jni_str!("equals"),
				jni_sig!("(Ljava/lang/Object;)Z"),
				&[JValue::from(&another_string)],
			)
			.expect("call nonvirtual method succeeds")
			.z()
			.expect("JNI result contains a boolean");
		assert!(!object_result);

		// However, java/lang/String overrided it and it now compares the content.
		let string_class: abos_jni::objects::JClass<'_> = env
			.find_class(jni_str!("java/lang/String"))
			.expect("test class can be resolved");
		let string_result: bool = env
			.call_nonvirtual_method(
				&a_string,
				&string_class,
				jni_str!("equals"),
				jni_sig!("(Ljava/lang/Object;)Z"),
				&[JValue::from(&another_string)],
			)
			.expect("call nonvirtual method succeeds")
			.z()
			.expect("JNI result contains a boolean");
		assert!(string_result);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn call_nonvirtual_method_rejects_incompatible_receiver() {
	attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let result: Result<abos_jni::JValueOwned<'_>, Error> =
			env.call_nonvirtual_method(&object, STRING_CLASS, jni_str!("length"), jni_sig!("()I"), &[]);
		assert_matches!(result, Err(Error::WrongObjectType));
		assert!(!env.exception_check());

		let string: JString<'_> = env.new_string("test")?;
		let integer_class: abos_jni::objects::JClass<'_> = env.find_class(INTEGER_CLASS)?;
		let result: Result<abos_jni::JValueOwned<'_>, Error> =
			env.call_nonvirtual_method(&string, &integer_class, jni_str!("intValue"), jni_sig!("()I"), &[]);
		assert_matches!(result, Err(Error::WrongObjectType));
		assert!(!env.exception_check());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn call_nonvirtual_method_rejects_null_receiver() {
	attach_current_thread(|env| {
		let result: Result<abos_jni::JValueOwned<'_>, Error> =
			env.call_nonvirtual_method(JObject::null(), STRING_CLASS, jni_str!("length"), jni_sig!("()I"), &[]);
		assert_matches!(result, Err(Error::NullPtr("call_nonvirtual_method obj argument")));
		assert!(!env.exception_check());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn short_lifetime_with_local_frame() {
	attach_current_thread(|env| {
		let object: std::result::Result<JObject<'_>, abos_jni::errors::Error> =
			short_lifetime_with_local_frame_sub_fn(env);
		assert!(object.is_ok());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn short_lifetime_with_local_frame_sub_fn<'local>(env: &'_ mut Env<'local>) -> Result<JObject<'local>, Error> {
	env.with_local_frame_returning_local::<_, JObject, _>(16, |env| {
		env.new_object(INTEGER_CLASS, jni_sig!("(I)V"), &[JValue::from(5)])
	})
}

#[test]
fn short_lifetime_list() {
	attach_current_thread(|env| {
		let first_list_object: JObject<'_> =
			short_lifetime_list_sub_fn(env).expect("short lifetime list sub fn succeeds");
		let value: std::result::Result<abos_jni::JValueOwned<'_>, abos_jni::errors::Error> =
			env.call_method(first_list_object, jni_str!("intValue"), jni_sig!("()I"), &[]);
		assert_eq!(
			value
				.expect("test operation succeeds")
				.i()
				.expect("JNI result contains an int"),
			1
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn short_lifetime_list_sub_fn<'local>(env: &'_ mut Env<'local>) -> Result<JObject<'local>, Error> {
	let list_object: JObject<'_> = env.new_object(ARRAYLIST_CLASS, jni_sig!("()V"), &[])?;
	let list: abos_jni::refs::Cast<'_, '_, JList<'_>> = env.as_cast::<JList>(&list_object)?;
	let element: JObject<'_> = env.new_object(INTEGER_CLASS, jni_sig!("(I)V"), &[JValue::from(1)])?;
	list.add(env, &element)?;
	short_lifetime_list_sub_fn_get_first_element(&list, env)
}

fn short_lifetime_list_sub_fn_get_first_element<'env_local>(
	list: &'_ JList<'_>,
	env: &'_ mut Env<'env_local>,
) -> Result<JObject<'env_local>, Error> {
	let iterator: abos_jni::objects::JIterator<'_> = list.iter(env)?;
	Ok(iterator.next(env)?.expect("next succeeds"))
}

#[test]
fn get_object_array_element() {
	attach_current_thread(|env| {
		let array: abos_jni::objects::JObjectArray<'_> = env
			.new_object_array(1, STRING_CLASS, JObject::null())
			.expect("new object array succeeds");

		assert!(!array.is_null());
		assert!(array.get_element(env, 0).expect("get element succeeds").is_null());
		let test_str: abos_jni::objects::JString<'_> = env.new_string("test").expect("test string can be allocated");
		array.set_element(env, 0, test_str).expect("set element succeeds");
		assert!(!array.get_element(env, 0).expect("get element succeeds").is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
