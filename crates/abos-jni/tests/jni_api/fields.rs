use super::*;

#[test]
pub fn get_public_field() {
	attach_current_thread(|env| {
		let point: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		let x_value: i32 = env
			.get_field(&point, jni_str!("x"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(x_value, 5);

		let y_value: i32 = env
			.get_field(&point, jni_str!("y"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(y_value, 10);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_public_field_by_id() {
	attach_current_thread(|env| {
		let point: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		let field_id: abos_jni::ids::JFieldID = env
			.get_field_id(jni_str!("java/awt/Point"), jni_str!("x"), jni_sig!("I"))
			.expect("get field id succeeds");

		let field_type: JavaType = ReturnType::Primitive(Primitive::Int);
		// Safety: we have just looked up the field ID based on the given class and field_type
		unsafe {
			let x_value: i32 = env
				.get_field_unchecked(&point, field_id, field_type)
				.expect("get field unchecked succeeds")
				.i()
				.expect("JNI result contains an int");

			assert_eq!(x_value, 5);
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn set_public_field() {
	attach_current_thread(|env| {
		let point: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		env.set_field(&point, jni_str!("x"), jni_sig!("I"), JValue::Int(15))
			.expect("test field can be assigned");

		let x_value: i32 = env
			.get_field(&point, jni_str!("x"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(x_value, 15);

		env.set_field(&point, jni_str!("y"), jni_sig!("I"), JValue::Int(25))
			.expect("test field can be assigned");

		let y_value: i32 = env
			.get_field(&point, jni_str!("y"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(y_value, 25);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn set_public_field_by_id() {
	attach_current_thread(|env| {
		let point: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		let field_id: abos_jni::ids::JFieldID = env
			.get_field_id(jni_str!("java/awt/Point"), jni_str!("x"), jni_sig!("I"))
			.expect("get field id succeeds");

		// Safety: we have just looked up the field ID based on the given field name and type
		unsafe {
			env.set_field_unchecked(&point, field_id, JValue::Int(15))
				.expect("set field unchecked succeeds");
		}

		let x_value: i32 = env
			.get_field(&point, jni_str!("x"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(x_value, 15);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

rusty_fork::rusty_fork_test! {
	#[test]
	fn set_array_field() {
		// Array values report JavaType::Object, but array signatures report JavaType::Array.
		// Field assignment must accept this combination.
		let out_dir: std::path::PathBuf = util::java_fixtures();
		attach_current_thread(|env| {
			util::load_test_class(env, &out_dir, "TestFields")?;

			let instance = env.new_object(jni_str!("com/example/TestFields"), jni_sig!("()V"), &[])?;

			let arr = env.new_byte_array(1)?;
			env.set_field(
				instance,
				jni_str!("byteArrayField"),
				jni_sig!("[B"),
				JValue::Object(&arr),
			)?;

			Ok(())
		})
		.expect("attached JNI callback succeeds");
	}

	#[test]
	fn set_static_array_field() {
		// Array values report JavaType::Object, but array signatures report JavaType::Array.
		// Static field assignment must accept this combination.
		let out_dir: std::path::PathBuf = util::java_fixtures();

		attach_current_thread(|env| {
			util::load_test_class(env, &out_dir, "TestFields")?;

			let class = env.load_class(jni_str!("com.example.TestFields"))?;

			let arr = env.new_byte_array(1)?;
			env.set_static_field(
				class,
				jni_str!("staticByteArray"),
				jni_sig!("[B"),
				JValue::Object(&arr),
			)?;

			Ok(())
		})
		.expect("attached JNI callback succeeds");
	}
}

#[test]
pub fn get_static_public_field() {
	attach_current_thread(|env| {
		let min_int_value: i32 = env
			.get_static_field(INTEGER_CLASS, jni_str!("MIN_VALUE"), jni_sig!("I"))
			.expect("test static field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(min_int_value, i32::MIN);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_static_public_field_by_id() {
	attach_current_thread(|env| {
		let field_id: abos_jni::ids::JStaticFieldID = env
			.get_static_field_id(INTEGER_CLASS, jni_str!("MIN_VALUE"), jni_sig!("I"))
			.expect("get static field id succeeds");

		let field_type: JavaType = JavaType::Primitive(Primitive::Int);
		// Safety: we have just looked up the field ID based on the given class and field_type
		unsafe {
			let min_int_value: i32 = env
				.get_static_field_unchecked(INTEGER_CLASS, field_id, field_type)
				.expect("get static field unchecked succeeds")
				.i()
				.expect("JNI result contains an int");

			assert_eq!(min_int_value, i32::MIN);
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

rusty_fork_test! {
#[test]
fn set_static_public_field() {
	attach_current_thread(|env| {
		// We'll use System.in which is a mutable static field

		let original_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		let byte_array = env.new_byte_array(10).expect("new byte array succeeds");
		let new_input_stream = env
			.new_object(
				jni_str!("java/io/ByteArrayInputStream"),
				jni_sig!("([B)V"),
				&[JValue::from(&byte_array)],
			)
			.expect("test object can be allocated");

		env.set_static_field(
			jni_str!("java/lang/System"),
			jni_str!("in"),
			jni_sig!("Ljava/io/InputStream;"),
			JValue::from(&new_input_stream),
		)
		.expect("set static field succeeds");

		let current_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		assert!(!current_in.is_null());

		env.set_static_field(
			jni_str!("java/lang/System"),
			jni_str!("in"),
			jni_sig!("Ljava/io/InputStream;"),
			JValue::from(&original_in),
		)
		.expect("set static field succeeds");

		let restored_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		assert!(!restored_in.is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
}

rusty_fork_test! {
#[test]
fn set_static_public_field_by_id() {
	attach_current_thread(|env| {
		let original_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		let field_type = jni_sig!("Ljava/io/InputStream;");
		let field_id = env
			.get_static_field_id(jni_str!("java/lang/System"), jni_str!("in"), field_type)
			.expect("get static field id succeeds");

		let byte_array = env.new_byte_array(10).expect("new byte array succeeds");
		let new_input_stream = env
			.new_object(
				jni_str!("java/io/ByteArrayInputStream"),
				jni_sig!("([B)V"),
				&[JValue::from(&byte_array)],
			)
			.expect("test object can be allocated");

		// Safety: we have just looked up the field ID based on the given field name and type
		unsafe {
			env.set_static_field_unchecked(
				jni_str!("java/lang/System"),
				field_id,
				JValue::from(&new_input_stream),
			)
			.expect("set static field unchecked succeeds");
		}

		let current_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		assert!(!current_in.is_null());

		// Safety: we have the correct field ID and value type
		unsafe {
			env.set_static_field_unchecked(
				jni_str!("java/lang/System"),
				field_id,
				JValue::from(&original_in),
			)
			.expect("set static field unchecked succeeds");
		}

		let restored_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		assert!(env.is_same_object(&original_in, &restored_in).expect("is same object succeeds"));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
}

/*
#[test]
pub fn pop_local_frame_pending_exception() {
attach_current_thread(|env| {
		env.push_local_frame(16).unwrap();

		env.throw_new(RUNTIME_EXCEPTION_CLASS, "Test Exception")
			.unwrap();

		// Pop the local frame with a pending exception
		unsafe { env.pop_local_frame(&JObject::null()) }
			.expect("Env#pop_local_frame must work in case of pending exception");

		env.exception_clear();

		Ok(())
	}).unwrap();
	}

#[test]
pub fn push_local_frame_pending_exception() {
attach_current_thread(|env| {
		env.throw_new(RUNTIME_EXCEPTION_CLASS, "Test Exception")
			.unwrap();

		// Push a new local frame with a pending exception
		env.push_local_frame(16)
			.expect("Env#push_local_frame must work in case of pending exception");

		env.exception_clear();

		unsafe { env.pop_local_frame(&JObject::null()) }.unwrap();

		Ok(())
	}).unwrap();
	}

#[test]
pub fn push_local_frame_too_many_refs() {
attach_current_thread(|env| {
		// Try to push a new local frame with a ridiculous size
		let frame_size = i32::MAX;
		env.push_local_frame(frame_size)
			.expect_err("push_local_frame(2B) must Err");

		unsafe { env.pop_local_frame(&JObject::null()) }.unwrap();

		Ok(())
	}).unwrap();
	}
*/
