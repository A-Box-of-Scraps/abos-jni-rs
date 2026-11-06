use super::*;

#[test]
pub fn attach_current_thread_guard_scope() {
	let mut scope = abos_jni::vm::ScopeToken::default();
	let (_vm, _guard) = {
		let vm = jvm();
		let vm = vm.as_ref().clone();
		// SAFETY: The scope token outlives the guard, which stays on this thread and is dropped in stack order.
		let guard = unsafe {
			vm.attach_current_thread_guard(Default::default, &mut scope)
				.expect("thread attaches to the test JVM")
		};
		vm.with_local_frame(10, |env| -> abos_jni::errors::Result<_> {
			let _s = env.new_string("hello").expect("test string can be allocated");
			Ok(())
		})
		.expect("local-frame callback succeeds");
		(vm, guard)
	};
}

#[test]
pub fn with_local_frame() {
	attach_current_thread(|env| {
		let s = env
			.with_local_frame_returning_local::<_, JObject, abos_jni::errors::Error>(16, |env| {
				let res = env.new_string("Test")?;
				Ok(res.into())
			})
			.expect("test string can be allocated");
		let s = env.cast_local::<JString>(s).expect("test operation succeeds");

		let s = s
			.mutf8_chars(env)
			.expect("The object returned from the local frame must remain valid");
		assert_eq!(s.to_str(), "Test");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
#[should_panic(expected = "not associated with the top JNI stack frame")]
pub fn with_local_frame_misuse_panic() {
	attach_current_thread(|env0| {
		env0.with_local_frame::<_, _, abos_jni::errors::Error>(5, |env1| {
			env0.with_local_frame::<_, _, abos_jni::errors::Error>(5, |_env2| {
				// At this point we have given ourselves access to mutable Env references (env1 and env2)
				// (env0 is tied up due to the `with_local_frame` calls)

				// As a failsafe though, we should panic if we attempt use env1 to create new local references
				// since env1 is not the top frame Env.
				let _res = env1.new_string("Test").expect("This should panic");

				Ok(())
			})
			.expect("test string can be allocated");
			Ok(())
		})
		.expect("test string can be allocated");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn with_local_frame_pending_exception() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw_new()` result since it will report Err(JavaException) but for the
		// test we want to continue regardless
		let res = env.throw_new(RUNTIME_EXCEPTION_CLASS, jni_str!("Test Exception"));
		assert!(matches!(res, Err(Error::JavaException)));

		// Try to allocate a frame of locals
		env.with_local_frame(16, |_| -> Result<_, Error> { Ok(()) })
			.expect("Env#with_local_frame must work in case of pending exception");

		env.exception_clear();

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_method_ok() {
	attach_current_thread(|env| {
		let s = JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");

		let v: jint = env
			.call_method(s, jni_str!("indexOf"), jni_sig!("(I)I"), &[JValue::Int('S' as i32)])
			.expect("Env#call_method should return JValue")
			.i()
			.expect("instance method call succeeds");

		assert_eq!(v, 2);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_method_with_bad_args_errs() {
	attach_current_thread(|env| {
		let s = JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");

		let is_bad_typ = env
			.call_method(
				&s,
				jni_str!("indexOf"),
				jni_sig!("(I)I"),
				&[JValue::Float(std::f32::consts::PI)],
			)
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#callmethod with bad arg type should err");

		assert!(
			is_bad_typ,
			"ErrorKind::InvalidArgList expected when passing bad value type"
		);

		let is_bad_len = env
			.call_method(
				&s,
				jni_str!("indexOf"),
				jni_sig!("(I)I"),
				&[JValue::Int('S' as i32), JValue::Long(3)],
			)
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#call_method with bad arg lengths should err");

		assert!(
			is_bad_len,
			"ErrorKind::InvalidArgList expected when passing bad argument lengths"
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_static_method_ok() {
	attach_current_thread(|env| {
		let x = JValue::from(-10);
		let val: jint = env
			.call_static_method(MATH_CLASS, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE, &[x])
			.expect("Env#call_static_method should return JValue")
			.i()
			.expect("static method call succeeds");

		assert_eq!(val, 10);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_static_method_unchecked_ok() {
	attach_current_thread(|env| {
		let x = JValue::from(-10);
		let math_class = env.find_class(MATH_CLASS).expect("test class can be resolved");
		let abs_method_id = env
			.get_static_method_id(&math_class, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE)
			.expect("test static method can be resolved");
		// SAFETY: The ID was looked up for Math.abs(int), and the argument and return types match.
		let val: jint = unsafe {
			env.call_static_method_unchecked(
				&math_class,
				abs_method_id,
				ReturnType::Primitive(Primitive::Int),
				&[x.as_jni()],
			)
		}
		.expect("Env#call_static_method_unchecked should return JValue")
		.i()
		.expect("call static method unchecked succeeds");

		assert_eq!(val, 10);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_new_object_unchecked_ok() {
	attach_current_thread(|env| {
		let test_str = JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");
		let string_class = env.find_class(STRING_CLASS).expect("test class can be resolved");

		let ctor_method_id = env
			.get_method_id(&string_class, jni_str!("<init>"), jni_sig!("(Ljava/lang/String;)V"))
			.expect("test method can be resolved");
		// SAFETY: The constructor ID belongs to String(String), and the live string argument matches that signature.
		let val: JObject =
			unsafe { env.new_object_unchecked(&string_class, ctor_method_id, &[JValue::from(&test_str).as_jni()]) }
				.expect("Env#new_object_unchecked should return JValue");

		let jstr = env.cast_local::<JString>(val).expect("test operation succeeds");
		let javastr = jstr.mutf8_chars(env).expect("modified UTF-8 characters are accessible");
		let jnistr: &JNIStr = javastr.as_ref();
		assert_eq!(jnistr, TESTING_OBJECT_STR);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_new_object_with_bad_args_errs() {
	attach_current_thread(|env| {
		let string_class = env.find_class(STRING_CLASS).expect("test class can be resolved");

		let is_bad_typ = env
			.new_object(&string_class, jni_sig!("(Ljava/lang/String;)V"), &[JValue::Int(2)])
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#new_object with bad arg type should err");

		assert!(
			is_bad_typ,
			"ErrorKind::InvalidArgList expected when passing bad value type"
		);

		let s = JString::from_jni_str(env, TESTING_OBJECT_STR).expect("test JNI string can be allocated");

		let is_bad_len = env
			.new_object(
				&string_class,
				jni_sig!("(Ljava/lang/String;)V"),
				&[JValue::from(&s), JValue::Int(2)],
			)
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#new_object with bad arg type should err");

		assert!(
			is_bad_len,
			"ErrorKind::InvalidArgList expected when passing bad argument lengths"
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

/// Check that we get a runtime error if trying to instantiate with an array class.
///
/// Although the JNI spec for `NewObjectA` states that the class "must not refer to an array class"
/// (and could therefor potentially trigger undefined behaviour if that rule is violated) we
/// expect that `Env::new_object()` shouldn't ever get as far as calling `NewObjectA` since
/// it will first fail (with a safe, runtime error) to lookup a method ID for any constructor.
/// (consistent with how [getConstructors()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/Class.html#getConstructors())
/// doesn't expose constructors for array classes)
#[test]
pub fn call_new_object_with_array_class() {
	attach_current_thread(|env| {
		let byte_array = env.new_byte_array(16).expect("new byte array succeeds");
		let array_class = env.get_object_class(byte_array).expect("get object class succeeds");
		// We just make up a plausible constructor signature
		let result = env.new_object(&array_class, jni_sig!("(I)[B"), &[JValue::Int(16)]);

		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_static_method_throws() {
	attach_current_thread(|env| {
		let x = JValue::Long(4_000_000_000);
		let is_java_exception = env
			.call_static_method(MATH_CLASS, MATH_TO_INT_METHOD_NAME, MATH_TO_INT_SIGNATURE, &[x])
			.map_err(|error| matches!(error, Error::JavaException))
			.expect_err("Env#call_static_method_unsafe should return error");

		// Throws a java.lang.ArithmeticException: integer overflow
		assert!(is_java_exception, "ErrorKind::JavaException expected as error");
		assert_pending_java_exception(env);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn call_static_method_with_bad_args_errs() {
	attach_current_thread(|env| {
		let x = JValue::Double(4.567_891_23);
		let is_bad_typ = env
			.call_static_method(MATH_CLASS, MATH_TO_INT_METHOD_NAME, MATH_TO_INT_SIGNATURE, &[x])
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#call_static_method with bad arg type should err");

		assert!(
			is_bad_typ,
			"ErrorKind::InvalidArgList expected when passing bad value type"
		);

		let is_bad_len = env
			.call_static_method(
				MATH_CLASS,
				MATH_TO_INT_METHOD_NAME,
				MATH_TO_INT_SIGNATURE,
				&[JValue::Int(2), JValue::Int(3)],
			)
			.map_err(|error| matches!(error, Error::InvalidArgList(_)))
			.expect_err("Env#call_static_method with bad arg lengths should err");

		assert!(
			is_bad_len,
			"ErrorKind::InvalidArgList expected when passing bad argument lengths"
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_reflected_method_from_id() {
	attach_current_thread(|env| {
		let ctor_method_id = env
			.get_method_id(INTEGER_CLASS, jni_str!("<init>"), jni_sig!("(Ljava/lang/String;)V"))
			.expect("constructor from string exists");
		let ctor = env
			.to_reflected_method(INTEGER_CLASS, ctor_method_id)
			.expect("to reflected method succeeds");

		let value = {
			let jstr = env.new_string("55").expect("test string can be allocated");
			let vargs = env
				.new_object_array(1, jni_str!("java/lang/Object"), jstr)
				.expect("can create array");

			env.call_method(
				ctor,
				jni_str!("newInstance"),
				jni_sig!("([Ljava/lang/Object;)Ljava/lang/Object;"),
				&[JValue::from(&vargs)],
			)
			.expect("can invoke Constructor.newInstance")
			.l()
			.expect("return value is an Integer")
		};

		let int_value = env
			.call_method(value, jni_str!("intValue"), jni_sig!("()I"), &[])
			.expect("instance method call succeeds")
			.i()
			.expect("JNI result contains an int");
		assert_eq!(int_value, 55);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_reflected_static_method_from_id() {
	attach_current_thread(|env| {
		let x = JValue::from(-10);
		let math_class = env.find_class(MATH_CLASS).expect("test class can be resolved");
		let abs_method_id = env
			.get_static_method_id(&math_class, MATH_ABS_METHOD_NAME, MATH_ABS_SIGNATURE)
			.expect("test static method can be resolved");

		let abs_method = env
			.to_reflected_static_method(&math_class, abs_method_id)
			.expect("to reflected static method succeeds");

		let arg = env
			.new_object(INTEGER_CLASS, jni_sig!("(I)V"), &[x])
			.expect("test object can be allocated");
		let vargs = env
			.new_object_array(1, jni_str!("java/lang/Object"), arg)
			.expect("new object array succeeds");
		let val = env
			.call_method(
				abs_method,
				jni_str!("invoke"),
				jni_sig!("(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;"),
				&[JValue::from(&JObject::null()), JValue::from(&vargs)],
			)
			.expect("can call Method.invoke")
			.l()
			.expect("return value is an int");
		let val = env
			.call_method(val, jni_str!("intValue"), jni_sig!("()I"), &[])
			.expect("instance method call succeeds")
			.i()
			.expect("JNI result contains an int");

		assert_eq!(val, 10);
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
