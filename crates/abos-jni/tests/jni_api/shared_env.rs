use super::*;
use abos_jni::{
	ids::{JFieldID, JMethodID, JStaticFieldID, JStaticMethodID},
	objects::JClass,
};

#[test]
fn resolved_class_operations_accept_shared_env() {
	attach_current_thread(|env| {
		let string: JClass<'_> = env.find_class(STRING_CLASS)?;
		let integer: JClass<'_> = env.find_class(INTEGER_CLASS)?;
		let point: JClass<'_> = env.find_class(jni_str!("java/awt/Point"))?;
		let exception: JClass<'_> = env.find_class(EXCEPTION_CLASS)?;
		let object: JString<'_> = env.new_string("shared")?;
		let method: JMethodID = env.get_method_id(&string, jni_str!("length"), jni_sig!("()I"))?;
		let static_method: JStaticMethodID =
			env.get_static_method_id(&integer, jni_str!("parseInt"), jni_sig!("(Ljava/lang/String;)I"))?;
		let field: JFieldID = env.get_field_id(&point, jni_str!("x"), jni_sig!("I"))?;
		let static_field: JStaticFieldID = env.get_static_field_id(&integer, jni_str!("MAX_VALUE"), jni_sig!("I"))?;
		let env: &Env<'_> = env;
		assert_eq!(
			env.get_method_id_from_class(&string, jni_str!("length"), jni_sig!("()I"))?
				.into_raw(),
			method.into_raw()
		);
		assert_eq!(
			env.get_static_method_id_from_class(&integer, jni_str!("parseInt"), jni_sig!("(Ljava/lang/String;)I"))?
				.into_raw(),
			static_method.into_raw()
		);
		assert_eq!(
			env.get_field_id_from_class(&point, jni_str!("x"), jni_sig!("I"))?
				.into_raw(),
			field.into_raw()
		);
		assert_eq!(
			env.get_static_field_id_from_class(&integer, jni_str!("MAX_VALUE"), jni_sig!("I"))?
				.into_raw(),
			static_field.into_raw()
		);
		assert!(env.is_assignable_from_class(&string, &string)?);
		assert!(!env.is_assignable_from_class(&string, &integer)?);
		assert!(env.is_instance_of_class(&object, &string)?);
		assert!(!env.is_instance_of_class(&object, &integer)?);
		assert!(env.is_instance_of_class(JObject::null(), &string)?);
		// SAFETY: An empty registration contains no function pointers or signatures.
		unsafe {
			env.register_native_methods_from_class(&string, &[])?;
		}
		assert_matches!(
			env.throw_new_from_class(&exception, TEST_EXCEPTION_MESSAGE),
			Err(Error::JavaException)
		);
		assert!(env.exception_check());
		env.exception_clear();
		Ok(())
	})
	.expect("shared environment operations succeed");
}

#[test]
fn resolved_class_lookup_errors_are_caught() {
	attach_current_thread(|env| {
		let class: JClass<'_> = env.find_class(STRING_CLASS)?;
		let env: &Env<'_> = env;
		assert_matches!(
			env.get_method_id_from_class(&class, jni_str!("missingMethod"), jni_sig!("()V")),
			Err(Error::MethodNotFound { .. })
		);
		assert!(!env.exception_check());
		assert_matches!(
			env.get_static_method_id_from_class(&class, jni_str!("missingMethod"), jni_sig!("()V")),
			Err(Error::MethodNotFound { .. })
		);
		assert!(!env.exception_check());
		assert_matches!(
			env.get_field_id_from_class(&class, jni_str!("missingField"), jni_sig!("I")),
			Err(Error::FieldNotFound { .. })
		);
		assert!(!env.exception_check());
		assert_matches!(
			env.get_static_field_id_from_class(&class, jni_str!("missingField"), jni_sig!("I")),
			Err(Error::FieldNotFound { .. })
		);
		assert!(!env.exception_check());
		assert_matches!(
			env.throw_new_from_class(&class, TEST_EXCEPTION_MESSAGE),
			Err(Error::WrongObjectType)
		);
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("lookup errors are translated");
}

#[test]
fn resolved_class_operations_reject_null_classes() {
	attach_current_thread(|env| {
		let class: JClass<'_> = JClass::null();
		let env: &Env<'_> = env;
		assert_matches!(
			env.get_method_id_from_class(&class, jni_str!("length"), jni_sig!("()I")),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			env.get_static_method_id_from_class(&class, jni_str!("length"), jni_sig!("()I")),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			env.get_field_id_from_class(&class, jni_str!("x"), jni_sig!("I")),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			env.get_static_field_id_from_class(&class, jni_str!("x"), jni_sig!("I")),
			Err(Error::NullPtr(_))
		);
		assert_matches!(env.is_assignable_from_class(&class, &class), Err(Error::NullPtr(_)));
		assert_matches!(
			env.is_instance_of_class(JObject::null(), &class),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			env.throw_new_from_class(&class, TEST_EXCEPTION_MESSAGE),
			Err(Error::NullPtr(_))
		);
		assert_matches!(
			// SAFETY: An empty registration contains no function pointers or signatures.
			unsafe { env.register_native_methods_from_class(&class, &[]) },
			Err(Error::NullPtr(_))
		);
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("null classes are rejected before JNI calls");
}

#[test]
fn resolved_class_operations_preserve_pending_exceptions() {
	attach_current_thread(|env| {
		let class: JClass<'_> = env.find_class(STRING_CLASS)?;
		let exception: JClass<'_> = env.find_class(EXCEPTION_CLASS)?;
		let env: &Env<'_> = env;
		assert_matches!(
			env.throw_new_from_class(&exception, TEST_EXCEPTION_MESSAGE),
			Err(Error::JavaException)
		);
		assert_matches!(
			env.get_method_id_from_class(&class, jni_str!("length"), jni_sig!("()I")),
			Err(Error::JavaException)
		);
		assert_matches!(
			env.get_static_method_id_from_class(&class, jni_str!("missing"), jni_sig!("()I")),
			Err(Error::JavaException)
		);
		assert_matches!(
			env.get_field_id_from_class(&class, jni_str!("missing"), jni_sig!("I")),
			Err(Error::JavaException)
		);
		assert_matches!(
			env.get_static_field_id_from_class(&class, jni_str!("missing"), jni_sig!("I")),
			Err(Error::JavaException)
		);
		assert_matches!(env.is_assignable_from_class(&class, &class), Err(Error::JavaException));
		assert_matches!(
			env.is_instance_of_class(JObject::null(), &class),
			Err(Error::JavaException)
		);
		assert_matches!(
			env.throw_new_from_class(&exception, TEST_EXCEPTION_MESSAGE),
			Err(Error::JavaException)
		);
		assert_matches!(
			// SAFETY: An empty registration contains no function pointers or signatures.
			unsafe { env.register_native_methods_from_class(&class, &[]) },
			Err(Error::JavaException)
		);
		assert!(env.exception_check());
		env.exception_clear();
		Ok(())
	})
	.expect("pending exceptions remain pending");
}
