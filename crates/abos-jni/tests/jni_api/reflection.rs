use super::*;

fn assert_declaring_class(env: &mut Env<'_>, reflected: &JObject<'_>, expected: &JNIStr) {
	let declaring_class: JObject<'_> = env
		.call_method(
			reflected,
			jni_str!("getDeclaringClass"),
			jni_sig!("()Ljava/lang/Class;"),
			&[],
		)
		.expect("reflected member has a declaring class")
		.l()
		.expect("declaring class is an object");
	let expected: abos_jni::objects::JClass<'_> = env.find_class(expected).expect("expected class exists");
	assert!(
		env.is_same_object(&declaring_class, &expected)
			.expect("class identity check succeeds")
	);
}

#[test]
fn reflect_instance_method() {
	attach_current_thread(|env| {
		let reflected: JObject<'_> = env.to_reflected_method(STRING_CLASS, jni_str!("length"), jni_sig!("()I"))?;
		assert_declaring_class(env, &reflected, STRING_CLASS);
		let string: JString<'_> = env.new_string("hello")?;
		let result: JObject<'_> = env
			.call_method(
				&reflected,
				jni_str!("invoke"),
				jni_sig!("(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;"),
				&[JValue::from(&string), JValue::from(&JObject::null())],
			)?
			.l()?;
		assert_eq!(
			env.call_method(result, jni_str!("intValue"), jni_sig!("()I"), &[])?
				.i()?,
			5
		);
		Ok(())
	})
	.expect("instance reflection succeeds");
}

#[test]
fn reflect_inherited_methods() {
	attach_current_thread(|env| {
		let reflected: JObject<'_> = env.to_reflected_method(
			jni_str!("java/lang/Thread"),
			jni_str!("getClass"),
			jni_sig!("()Ljava/lang/Class;"),
		)?;
		assert_declaring_class(env, &reflected, jni_str!("java/lang/Object"));
		let reflected: JObject<'_> = env.to_reflected_static_method(
			jni_str!("java/sql/Date"),
			jni_str!("parse"),
			jni_sig!("(Ljava/lang/String;)J"),
		)?;
		assert_declaring_class(env, &reflected, jni_str!("java/util/Date"));
		Ok(())
	})
	.expect("inherited reflection succeeds");
}

#[test]
fn reflection_rejects_methods_missing_from_supplied_class() {
	attach_current_thread(|env| {
		let thread: abos_jni::objects::JClass<'_> = env.find_class(jni_str!("java/lang/Thread"))?;
		assert_matches!(
			env.to_reflected_method(&thread, jni_str!("length"), jni_sig!("()I")),
			Err(Error::MethodNotFound { .. })
		);
		assert!(!env.exception_check());
		assert_matches!(
			env.to_reflected_static_method(&thread, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE),
			Err(Error::MethodNotFound { .. })
		);
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("incompatible lookups return errors without aborting");
}

#[test]
fn reflection_rejects_wrong_signatures_and_staticness() {
	attach_current_thread(|env| {
		assert_matches!(
			env.to_reflected_method(STRING_CLASS, jni_str!("length"), jni_sig!("()J")),
			Err(Error::MethodNotFound { .. })
		);
		assert_matches!(
			env.to_reflected_static_method(MATH_CLASS, MATH_ABS_METHOD_NAME, jni_sig!("(I)J")),
			Err(Error::MethodNotFound { .. })
		);
		assert_matches!(
			env.to_reflected_static_method(STRING_CLASS, jni_str!("length"), jni_sig!("()I")),
			Err(Error::MethodNotFound { .. })
		);
		assert_matches!(
			env.to_reflected_method(MATH_CLASS, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE),
			Err(Error::MethodNotFound { .. })
		);
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("invalid method lookups return errors");
}

#[test]
fn reflection_rejects_null_class() {
	attach_current_thread(|env| {
		let class: abos_jni::objects::JClass<'_> = abos_jni::objects::JClass::null();
		assert_matches!(
			env.to_reflected_method(&class, jni_str!("length"), jni_sig!("()I")),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			env.to_reflected_static_method(&class, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE),
			Err(Error::NullPtr(_))
		);
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("null class is rejected before reflection");
}

#[test]
fn reflect_cached_method_ids_unchecked() {
	attach_current_thread(|env| {
		let string: abos_jni::objects::JClass<'_> = env.find_class(STRING_CLASS)?;
		let id: abos_jni::ids::JMethodID = env.get_method_id(&string, jni_str!("length"), jni_sig!("()I"))?;
		// SAFETY: The ID was resolved from this class, whose local reference remains live.
		let reflected: JObject<'_> = unsafe { env.to_reflected_method_unchecked(&string, id)? };
		assert_declaring_class(env, &reflected, STRING_CLASS);

		let id: abos_jni::ids::JMethodID = env.get_method_id(&string, jni_str!("<init>"), jni_sig!("()V"))?;
		// SAFETY: The constructor ID was resolved from this live class.
		let reflected: JObject<'_> = unsafe { env.to_reflected_method_unchecked(&string, id)? };
		assert!(env.is_instance_of(&reflected, jni_str!("java/lang/reflect/Constructor"))?);
		assert_declaring_class(env, &reflected, STRING_CLASS);

		let math: abos_jni::objects::JClass<'_> = env.find_class(MATH_CLASS)?;
		let id: abos_jni::ids::JStaticMethodID =
			env.get_static_method_id(&math, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE)?;
		// SAFETY: The static ID was resolved from this class, whose local reference remains live.
		let reflected: JObject<'_> = unsafe { env.to_reflected_static_method_unchecked(&math, id)? };
		assert_declaring_class(env, &reflected, MATH_CLASS);
		Ok(())
	})
	.expect("cached method reflection succeeds");
}
