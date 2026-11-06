use super::*;

#[test]
pub fn get_public_field() {
	attach_current_thread(|env| {
		// Create a new Point(5, 10)
		let point = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		// Get the x field value
		let x_value = env
			.get_field(&point, jni_str!("x"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(x_value, 5);

		// Get the y field value
		let y_value = env
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
		// Create a new Point(5, 10)
		let point = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		// Get the field ID for x field
		let field_id = env
			.get_field_id(jni_str!("java/awt/Point"), jni_str!("x"), jni_sig!("I"))
			.expect("get field id succeeds");

		let field_type = ReturnType::Primitive(Primitive::Int);
		// Safety: we have just looked up the field ID based on the given class and field_type
		unsafe {
			let x_value = env
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
		// Create a new Point(5, 10)
		let point = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		// Set the x field to a new value
		env.set_field(&point, jni_str!("x"), jni_sig!("I"), JValue::Int(15))
			.expect("test field can be assigned");

		// Verify the field was set
		let x_value = env
			.get_field(&point, jni_str!("x"), jni_sig!("I"))
			.expect("test field can be read")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(x_value, 15);

		// Set the y field to a new value
		env.set_field(&point, jni_str!("y"), jni_sig!("I"), JValue::Int(25))
			.expect("test field can be assigned");

		// Verify the field was set
		let y_value = env
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
		// Create a new Point(5, 10)
		let point = unwrap(
			env.new_object(
				jni_str!("java/awt/Point"),
				jni_sig!("(II)V"),
				&[JValue::Int(5), JValue::Int(10)],
			),
			env,
		)
		.auto();

		// Get the field ID for x field
		let field_id = env
			.get_field_id(jni_str!("java/awt/Point"), jni_str!("x"), jni_sig!("I"))
			.expect("get field id succeeds");

		// Set the x field using the field ID
		// Safety: we have just looked up the field ID based on the given field name and type
		unsafe {
			env.set_field_unchecked(&point, field_id, JValue::Int(15))
				.expect("set field unchecked succeeds");
		}

		// Verify the field was set
		let x_value = env
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
		// This test reproduces an issue where the `set_field` threw an error when
		// setting an array field. The reason this was happening is that `set_field`
		// checks that the type of the incoming `JValue` matches the type of the
		// field, but the type of an array returned by `new_byte_array` is
		// `JavaType::Object` whilst the type of an array signature is
		// `JavaType::Array`.
		let out_dir = util::setup_test_output("set_array_field");

		let _classes = abos_javac::Build::new()
			.file("tests/java/com/example/TestFields.java")
			.output_dir(&out_dir)
			.compile();
		attach_current_thread(|env| {
			util::load_test_class(env, &out_dir, "TestFields")?;

			let instance = env.new_object(jni_str!("com/example/TestFields"), jni_sig!("()V"), &[])?;

			// Set the `staticByteArray` field to a new value
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
		// This test reproduces an issue where the `set_static_field` threw an error when
		// setting an array field. The reason this was happening is that `set_static_field`
		// checks that the type of the incoming `JValue` matches the type of the
		// field, but the type of an array returned by `new_byte_array` is
		// `JavaType::Object` whilst the type of an array signature is
		// `JavaType::Array`.
		let out_dir = util::setup_test_output("set_static_array_field");

		let _classes = abos_javac::Build::new()
			.file("tests/java/com/example/TestFields.java")
			.output_dir(&out_dir)
			.compile();

		attach_current_thread(|env| {
			util::load_test_class(env, &out_dir, "TestFields")?;

			// let instance = env.new_object(jni_str!("com/example/TestFields"), jni_sig!("()V"), &[])?;
			let class = env.load_class(jni_str!("com/example/TestFields"))?;

			// Set the `staticByteArray` field to a new value
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
		let min_int_value = env
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
		let field_id = env
			.get_static_field_id(INTEGER_CLASS, jni_str!("MIN_VALUE"), jni_sig!("I"))
			.expect("get static field id succeeds");

		let field_type = JavaType::Primitive(Primitive::Int);
		// Safety: we have just looked up the field ID based on the given class and field_type
		unsafe {
			let min_int_value = env
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

		// Get the original System.in value
		let original_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		// Create a new ByteArrayInputStream as a different InputStream
		let byte_array = env.new_byte_array(10).expect("new byte array succeeds");
		let new_input_stream = env
			.new_object(
				jni_str!("java/io/ByteArrayInputStream"),
				jni_sig!("([B)V"),
				&[JValue::from(&byte_array)],
			)
			.expect("test object can be allocated");

		// Set System.in to our new ByteArrayInputStream
		env.set_static_field(
			jni_str!("java/lang/System"),
			jni_str!("in"),
			jni_sig!("Ljava/io/InputStream;"),
			JValue::from(&new_input_stream),
		)
		.expect("set static field succeeds");

		// Verify the field was set by getting it again and checking it's no longer null
		let current_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		// The field should not be null after setting
		assert!(!current_in.is_null());

		// Restore the original System.in
		env.set_static_field(
			jni_str!("java/lang/System"),
			jni_str!("in"),
			jni_sig!("Ljava/io/InputStream;"),
			JValue::from(&original_in),
		)
		.expect("set static field succeeds");

		// Verify restoration worked - the field should still not be null
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
		// Get the original System.in value
		let original_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		// Get the field ID for System.in
		let field_type = jni_sig!("Ljava/io/InputStream;");
		let field_id = env
			.get_static_field_id(jni_str!("java/lang/System"), jni_str!("in"), field_type)
			.expect("get static field id succeeds");

		// Create a new ByteArrayInputStream as a different InputStream
		let byte_array = env.new_byte_array(10).expect("new byte array succeeds");
		let new_input_stream = env
			.new_object(
				jni_str!("java/io/ByteArrayInputStream"),
				jni_sig!("([B)V"),
				&[JValue::from(&byte_array)],
			)
			.expect("test object can be allocated");

		// Set System.in to our new ByteArrayInputStream using the field ID
		// Safety: we have just looked up the field ID based on the given field name and type
		unsafe {
			env.set_static_field_unchecked(
				jni_str!("java/lang/System"),
				field_id,
				JValue::from(&new_input_stream),
			)
			.expect("set static field unchecked succeeds");
		}

		// Verify the field was set by getting it again (this ensures the set operation worked)
		let current_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		// Verify that we can successfully retrieve a non-null input stream
		assert!(!current_in.is_null());

		// Restore the original System.in using the unchecked method
		// Safety: we have the correct field ID and value type
		unsafe {
			env.set_static_field_unchecked(
				jni_str!("java/lang/System"),
				field_id,
				JValue::from(&original_in),
			)
			.expect("set static field unchecked succeeds");
		}

		// Verify restoration worked
		let restored_in = env
			.get_static_field(jni_str!("java/lang/System"), jni_str!("in"), jni_sig!("Ljava/io/InputStream;"))
			.expect("test static field can be read")
			.l()
			.expect("JNI result contains an object");

		// The restored value should be the same as original
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
