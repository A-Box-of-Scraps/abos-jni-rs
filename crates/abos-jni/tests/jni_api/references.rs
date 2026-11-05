use super::*;

#[test]
fn get_super_class_ok() {
	attach_current_thread(|env| {
		let result = env.get_superclass(ARRAYLIST_CLASS);
		assert!(result.is_ok());
		assert!(result.unwrap().is_some());

		Ok(())
	})
	.unwrap();
}

#[test]
fn get_super_class_null() {
	attach_current_thread(|env| {
		let result = env.get_superclass(jni_str!("java/lang/Object"));
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());

		Ok(())
	})
	.unwrap();
}

#[test]
fn local_ref_null() {
	attach_current_thread(|env| {
		let null_obj = JObject::null();

		let result = env.new_local_ref::<&JObject>(&null_obj);
		assert!(result.is_ok());
		assert!(result.unwrap().is_null());

		// "delete" null reference
		env.delete_local_ref(null_obj);

		Ok(())
	})
	.unwrap();
}

#[test]
fn new_global_ref_null() {
	attach_current_thread(|env| {
		let null_obj = JObject::null();
		let result = env.new_global_ref(null_obj);
		assert!(result.is_ok());
		assert!(result.unwrap().is_null());

		Ok(())
	})
	.unwrap();
}

#[test]
fn new_weak_ref_null() {
	attach_current_thread(|env| {
		let null_obj = JObject::null();
		let result = env.new_weak_ref(null_obj);
		assert!(matches!(result, Err(Error::ObjectFreed)));

		let null_weak: Weak<JObject<'static>> = Weak::null();
		assert!(null_weak.is_garbage_collected(env).unwrap());

		Ok(())
	})
	.unwrap();
}

#[test]
fn auto_null() {
	let null_obj = JObject::null();
	{
		let auto_ref = null_obj.auto();
		assert!(auto_ref.is_null());
	}
}

#[test]
fn test_call_nonvirtual_method() {
	attach_current_thread(|env| {
		let a_string = JObject::from(env.new_string("test").unwrap());
		let another_string = JObject::from(env.new_string("test").unwrap());

		// The `equals` method in java/lang/Object will compare the reference
		let obj_class = env.find_class(jni_str!("java/lang/Object")).unwrap();
		let object_result = env
			.call_nonvirtual_method(
				&a_string,
				&obj_class,
				jni_str!("equals"),
				jni_sig!("(Ljava/lang/Object;)Z"),
				&[JValue::from(&another_string)],
			)
			.unwrap()
			.z()
			.unwrap();
		assert!(!object_result);

		// However, java/lang/String overrided it and it now compares the content.
		let string_class = env.find_class(jni_str!("java/lang/String")).unwrap();
		let string_result = env
			.call_nonvirtual_method(
				&a_string,
				&string_class,
				jni_str!("equals"),
				jni_sig!("(Ljava/lang/Object;)Z"),
				&[JValue::from(&another_string)],
			)
			.unwrap()
			.z()
			.unwrap();
		assert!(string_result);

		Ok(())
	})
	.unwrap();
}

#[test]
fn short_lifetime_with_local_frame() {
	attach_current_thread(|env| {
		let object = short_lifetime_with_local_frame_sub_fn(env);
		assert!(object.is_ok());

		Ok(())
	})
	.unwrap();
}

fn short_lifetime_with_local_frame_sub_fn<'local>(env: &'_ mut Env<'local>) -> Result<JObject<'local>, Error> {
	env.with_local_frame_returning_local::<_, JObject, _>(16, |env| {
		env.new_object(INTEGER_CLASS, jni_sig!("(I)V"), &[JValue::from(5)])
	})
}

#[test]
fn short_lifetime_list() {
	attach_current_thread(|env| {
		let first_list_object = short_lifetime_list_sub_fn(env).unwrap();
		let value = env.call_method(first_list_object, jni_str!("intValue"), jni_sig!("()I"), &[]);
		assert_eq!(value.unwrap().i().unwrap(), 1);

		Ok(())
	})
	.unwrap();
}

fn short_lifetime_list_sub_fn<'local>(env: &'_ mut Env<'local>) -> Result<JObject<'local>, Error> {
	let list_object = env.new_object(ARRAYLIST_CLASS, jni_sig!("()V"), &[])?;
	let list = env.as_cast::<JList>(&list_object)?;
	let element = env.new_object(INTEGER_CLASS, jni_sig!("(I)V"), &[JValue::from(1)])?;
	list.add(env, &element)?;
	short_lifetime_list_sub_fn_get_first_element(&list, env)
}

fn short_lifetime_list_sub_fn_get_first_element<'list_local, 'env_local>(
	list: &'_ JList<'list_local>,
	env: &'_ mut Env<'env_local>,
) -> Result<JObject<'env_local>, Error> {
	let iterator = list.iter(env)?;
	Ok(iterator.next(env)?.unwrap())
}

#[test]
fn get_object_array_element() {
	attach_current_thread(|env| {
		let array = env.new_object_array(1, STRING_CLASS, JObject::null()).unwrap();

		assert!(!array.is_null());
		assert!(array.get_element(env, 0).unwrap().is_null());
		let test_str = env.new_string("test").unwrap();
		array.set_element(env, 0, test_str).unwrap();
		assert!(!array.get_element(env, 0).unwrap().is_null());

		Ok(())
	})
	.unwrap();
}
