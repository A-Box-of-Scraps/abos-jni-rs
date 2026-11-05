use super::*;

#[test]
pub fn throw_new() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw_new()` result since it will report Err(JavaException) but for the
		// test we want to continue regardless
		let res = env.throw_new(RUNTIME_EXCEPTION_CLASS, jni_str!("Test Exception"));
		assert!(matches!(res, Err(Error::JavaException)));
		assert_pending_java_exception_detailed(env, Some(RUNTIME_EXCEPTION_CLASS), Some(jni_str!("Test Exception")));

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn throw_new_void() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw_new_void()` result since it will report Err(JavaException) but for the
		// test we want to continue regardless
		let res = env.throw_new_void(RUNTIME_EXCEPTION_CLASS);
		assert!(matches!(res, Err(Error::JavaException)));

		assert!(env.exception_check());
		let exception = env.exception_occurred().expect("Unable to get exception");
		env.exception_clear();

		assert_exception_type(env, &exception, RUNTIME_EXCEPTION_CLASS);

		let message = exception.get_message(env).unwrap();
		assert!(message.is_null());

		Ok(())
	})
	.unwrap();
}

#[test]
#[should_panic(expected = "WrongObjectType")]
pub fn throw_new_non_throwable_class() {
	attach_current_thread(|env| {
		let string_class = env.find_class(jni_str!("java/lang/String")).unwrap();

		env.throw_new(string_class, jni_str!("Test Exception")).unwrap();

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn throw_new_fail() {
	attach_current_thread(|env| {
		// Note: this isn't a great test because it's really just testing
		// what error we get from `Desc<JClass>::lookup` when looking up
		// an invalid class - it doesn't really have anything to do with
		// making a `ThrowNew` JNI call.
		let result = env.throw_new(jni_str!("java/lang/NonexistentException"), jni_str!("Test Exception"));
		assert!(result.is_err());
		eprintln!("result = {:?}", result);

		assert!(matches!(result, Err(Error::NoClassDefFound { .. })));

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn throw_defaults() {
	attach_current_thread(|env| {
		test_throwable_descriptor_with_default_type(env, TEST_EXCEPTION_MESSAGE);
		test_throwable_descriptor_with_default_type(env, TEST_EXCEPTION_MESSAGE.to_owned());
		test_throwable_descriptor_with_default_type(env, JNIString::from(TEST_EXCEPTION_MESSAGE));

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn test_conversion() {
	attach_current_thread(|env| {
		let orig_obj: JObject = env.new_string("Hello, world!").unwrap().into();

		let obj: JObject = unwrap(env.new_local_ref(&orig_obj), env);

		let string = env.cast_local::<JString>(obj).unwrap();
		let actual = JObject::from(string);
		assert!(env.is_same_object(&orig_obj, actual).unwrap());

		let global_ref = env.new_global_ref(&orig_obj).unwrap();
		assert!(env.is_same_object(&orig_obj, global_ref).unwrap());

		let weak_ref = unwrap(env.new_weak_ref(&orig_obj), env);
		let actual = unwrap(weak_ref.upgrade_local(env), env).expect("weak ref should not have been GC'd");
		assert!(env.is_same_object(&orig_obj, actual).unwrap());

		let auto_local = unwrap(env.new_local_ref(&orig_obj), env).auto();
		assert!(env.is_same_object(&orig_obj, auto_local).unwrap());

		Ok(())
	})
	.unwrap();
}

rusty_fork_test! {
#[test]
fn test_jstring_conversion() {
	// Even while JNI is not initialized a nul should be formatted as "<NULL>"
	let null = JString::null();
	assert_eq!(null.to_string(), "<NULL>");

	// XXX: this is highly unsafe but the expectation here is that the implementation
	// will not get as far as attempting to dereference the invalid pointer because
	// JavaVM::singleton is not yet initialized.
	//
	// Alternatively the only other way we could test this case would be with a
	// native method callback which is hard to reproduce here.
	let invalid = unsafe { JString::kind_from_raw(1 as _) };
	assert_eq!(invalid.to_string(), "<JNI Not Initialized>");

	attach_current_thread(|env| {
		let hello: JString = env.new_string("Hello, world!").unwrap();

		assert_eq!(hello.to_string(), "Hello, world!");
		assert_eq!(hello.to_string(), hello.try_to_string(env).unwrap());

		Ok(())
	})
	.unwrap();
}
}

#[test]
pub fn test_null_string_mutf8_chars() {
	attach_current_thread(|env| {
		let s = unsafe { JString::from_raw(env, std::ptr::null_mut() as _) };
		let ret = s.mutf8_chars(env);
		assert!(ret.is_err());

		Ok(())
	})
	.unwrap();
}

fn test_throwable_descriptor_with_default_type<'local, D>(env: &mut Env<'local>, descriptor: D)
where
	D: Desc<'local, JThrowable<'local>>,
{
	let result = descriptor.lookup(env);
	assert!(result.is_ok());
	let exception = result.unwrap();
	let exception = exception.as_ref();

	assert_exception_type(env, exception, RUNTIME_EXCEPTION_CLASS);
	assert_exception_message(env, exception, TEST_EXCEPTION_MESSAGE);
}

#[test]
fn test_throwable_get_stack_trace() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw()` result since it will report Err(JavaException) but for the test
		// we want to continue regardless
		let res = env.throw("Test exception");
		assert!(matches!(res, Err(Error::JavaException)));
		let exception = env.exception_occurred().unwrap();
		env.exception_clear();

		let stack_trace = exception.get_stack_trace(env).unwrap();
		let _len = stack_trace.len(env).unwrap();

		// Test conversion to JObjectArray<JObject> and back
		let obj_arr = env.cast_local::<JObjectArray>(stack_trace).unwrap();

		let stack_trace = env.as_cast::<JObjectArray<JStackTraceElement>>(&obj_arr).unwrap();
		let stack_trace = stack_trace.as_ref();
		let len = stack_trace.len(env).unwrap();

		// XXX: we can't actually test walking the stack trace without some Java code
		assert_eq!(len, 0);

		for i in 0..len {
			let element = stack_trace.get_element(env, i).unwrap();
			let _class_name = element.get_class_name(env).unwrap();
			let _method_name = element.get_method_name(env).unwrap();
			let _file_name = element.get_file_name(env).unwrap();
			let _line_number = element.get_line_number(env).unwrap();
			let _s = element.try_to_string(env).unwrap();

			// Do something with the stack trace element
		}

		Ok(())
	})
	.unwrap();
}

#[test]
fn test_exception_match() {
	attach_current_thread(|env| {
		let jstring_not_numeric = env.new_string("NOT A NUMBER").unwrap();

		let err_result = env.call_static_method(
			abos_jni::jni_str!("java/lang/Integer"),
			abos_jni::jni_str!("parseInt"),
			abos_jni::jni_sig!((str: JString) -> jint),
			&[(&jstring_not_numeric).into()],
		);
		assert!(matches!(err_result, Err(abos_jni::errors::Error::JavaException)));

		let res = if env.exception_check() {
			env.with_local_frame(16, |env| -> abos_jni::errors::Result<()> {
				let e = env
					.exception_occurred()
					.expect("Expected an exception after ExceptionCheck");
				env.exception_clear();

				if let Some(_oom) = abos_jni::exceptions::JOutOfMemoryError::matches(env, &e)? {
					Err(abos_jni::errors::Error::JniCall(JniError::NoMemory))
				} else if let Some(eformat) = abos_jni::exceptions::JNumberFormatException::matches(env, &e)? {
					let msg = eformat.as_throwable().get_message(env).unwrap_or_default();
					let msg = msg.to_string();
					Err(Error::ParseFailed(msg))
				} else {
					eprintln!("Unexpected exception: {:?}", e);
					Err(Error::JniCall(JniError::Unknown))
				}
			})
		} else {
			Ok(())
		};

		eprintln!("Caught exception: {:?}", res);
		assert!(matches!(res, Err(Error::ParseFailed(_))));

		Ok(())
	})
	.unwrap();
}
