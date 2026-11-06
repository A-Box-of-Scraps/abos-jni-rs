use super::*;
use abos_jni::objects::JClass;

#[test]
pub fn throw_new() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw_new()` result since it will report Err(JavaException) but for the
		// test we want to continue regardless
		let res: std::result::Result<(), abos_jni::errors::Error> =
			env.throw_new(RUNTIME_EXCEPTION_CLASS, jni_str!("Test Exception"));
		assert!(matches!(res, Err(Error::JavaException)));
		assert_pending_java_exception_detailed(env, Some(RUNTIME_EXCEPTION_CLASS), Some(jni_str!("Test Exception")));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn throw_new_void() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw_new_void()` result since it will report Err(JavaException) but for the
		// test we want to continue regardless
		let res: std::result::Result<(), abos_jni::errors::Error> = env.throw_new_void(RUNTIME_EXCEPTION_CLASS);
		assert!(matches!(res, Err(Error::JavaException)));

		assert!(env.exception_check());
		let exception: JThrowable<'_> = env.exception_occurred().expect("Unable to get exception");
		env.exception_clear();

		assert_exception_type(env, &exception, RUNTIME_EXCEPTION_CLASS);

		let message: JString<'_> = exception.get_message(env).expect("get message succeeds");
		assert!(message.is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
#[should_panic(expected = "WrongObjectType")]
pub fn throw_new_non_throwable_class() {
	attach_current_thread(|env| {
		let string_class: JClass<'_> = env
			.find_class(jni_str!("java/lang/String"))
			.expect("test class can be resolved");

		env.throw_new(string_class, jni_str!("Test Exception"))
			.expect("throw new succeeds");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn throw_new_fail() {
	attach_current_thread(|env| {
		// Note: this isn't a great test because it's really just testing
		// what error we get from `Desc<JClass>::lookup` when looking up
		// an invalid class - it doesn't really have anything to do with
		// making a `ThrowNew` JNI call.
		let result: std::result::Result<(), abos_jni::errors::Error> =
			env.throw_new(jni_str!("java/lang/NonexistentException"), jni_str!("Test Exception"));
		assert!(result.is_err());
		eprintln!("result = {result:?}");

		assert!(matches!(result, Err(Error::NoClassDefFound { .. })));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn throw_defaults() {
	attach_current_thread(|env| {
		test_throwable_descriptor_with_default_type(env, TEST_EXCEPTION_MESSAGE);
		test_throwable_descriptor_with_default_type(env, TEST_EXCEPTION_MESSAGE.to_owned());
		test_throwable_descriptor_with_default_type(env, JNIString::from(TEST_EXCEPTION_MESSAGE));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn test_conversion() {
	attach_current_thread(|env| {
		let orig_obj: JObject = env
			.new_string("Hello, world!")
			.expect("test string can be allocated")
			.into();

		let obj: JObject = unwrap(env.new_local_ref(&orig_obj), env);

		let string: JString<'_> = env.cast_local::<JString>(obj).expect("test operation succeeds");
		let actual: JObject<'_> = JObject::from(string);
		assert!(env.is_same_object(&orig_obj, actual).expect("is same object succeeds"));

		let global_ref: abos_jni::refs::Global<JObject<'_>> = env
			.new_global_ref(&orig_obj)
			.expect("global reference can be allocated");
		assert!(
			env.is_same_object(&orig_obj, global_ref)
				.expect("is same object succeeds")
		);

		let weak_ref: abos_jni::refs::Weak<JObject<'_>> = unwrap(env.new_weak_ref(&orig_obj), env);
		let actual: JObject<'_> = unwrap(weak_ref.upgrade_local(env), env).expect("weak ref should not have been GC'd");
		assert!(env.is_same_object(&orig_obj, actual).expect("is same object succeeds"));

		let auto_local: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(env.new_local_ref(&orig_obj), env).auto();
		assert!(
			env.is_same_object(&orig_obj, auto_local)
				.expect("is same object succeeds")
		);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

rusty_fork_test! {
#[test]
fn test_jstring_conversion() {
	// Even while JNI is not initialized a nul should be formatted as "<NULL>"
	let null = JString::null();
	assert_eq!(null.to_string(), "<NULL>");

	assert!(abos_jni::JavaVM::singleton().is_err());
	// SAFETY: In this isolated pre-initialization process, Display returns before using the
	// sentinel. It is never dereferenced or passed to JNI, and is discarded before JVM setup.
	let invalid = unsafe { JString::kind_from_raw(1 as _) };
	assert_eq!(invalid.to_string(), "<JNI Not Initialized>");

	attach_current_thread(|env| {
		let hello: JString = env.new_string("Hello, world!").expect("test string can be allocated");

		assert_eq!(hello.to_string(), "Hello, world!");
		assert_eq!(hello.to_string(), hello.try_to_string(env).expect("test Java string can be converted to Rust"));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
}

#[test]
pub fn test_null_string_mutf8_chars() {
	attach_current_thread(|env| {
		let s: JString<'_> = JString::null();
		let ret: std::result::Result<abos_jni::strings::MUTF8Chars<'_, &JString<'_>>, abos_jni::errors::Error> =
			s.mutf8_chars(env);
		assert!(ret.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn test_throwable_descriptor_with_default_type<'local, D>(env: &mut Env<'local>, descriptor: D)
where
	D: Desc<'local, JThrowable<'local>>,
{
	let result: std::result::Result<<D as Desc<'_, JThrowable<'_>>>::Output, abos_jni::errors::Error> =
		descriptor.lookup(env);
	assert!(result.is_ok());
	let exception: <D as Desc<'_, JThrowable<'_>>>::Output = result.expect("test operation succeeds");
	let exception: &JThrowable<'_> = exception.as_ref();

	assert_exception_type(env, exception, RUNTIME_EXCEPTION_CLASS);
	assert_exception_message(env, exception, TEST_EXCEPTION_MESSAGE);
}

#[test]
fn test_throwable_get_stack_trace() {
	attach_current_thread(|env| {
		// Intentionally avoid unwrapping the `.throw()` result since it will report Err(JavaException) but for the test
		// we want to continue regardless
		let res: std::result::Result<(), abos_jni::errors::Error> = env.throw("Test exception");
		assert!(matches!(res, Err(Error::JavaException)));
		let exception: JThrowable<'_> = env.exception_occurred().expect("exception occurred succeeds");
		env.exception_clear();

		let stack_trace: JObjectArray<'_, JStackTraceElement<'_>> =
			exception.get_stack_trace(env).expect("get stack trace succeeds");
		let _len: usize = stack_trace.len(env).expect("array length is available");

		// Test conversion to JObjectArray<JObject> and back
		let obj_arr: JObjectArray<'_> = env
			.cast_local::<JObjectArray>(stack_trace)
			.expect("test operation succeeds");

		let stack_trace: abos_jni::refs::Cast<'_, '_, JObjectArray<'_, JStackTraceElement<'_>>> = env
			.as_cast::<JObjectArray<JStackTraceElement>>(&obj_arr)
			.expect("test operation succeeds");
		let stack_trace: &JObjectArray<'_, JStackTraceElement<'_>> = stack_trace.as_ref();
		let len: usize = stack_trace.len(env).expect("array length is available");

		// XXX: we can't actually test walking the stack trace without some Java code
		assert_eq!(len, 0);

		for i in 0..len {
			let element: JStackTraceElement<'_> = stack_trace.get_element(env, i).expect("get element succeeds");
			let _class_name: JString<'_> = element.get_class_name(env).expect("get class name succeeds");
			let _method_name: JString<'_> = element.get_method_name(env).expect("get method name succeeds");
			let _file_name: JString<'_> = element.get_file_name(env).expect("get file name succeeds");
			let _line_number: i32 = element.get_line_number(env).expect("get line number succeeds");
			let _s: JString<'_> = element
				.try_to_string(env)
				.expect("test Java string can be converted to Rust");

			// Do something with the stack trace element
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_exception_match() {
	attach_current_thread(|env| {
		let jstring_not_numeric: JString<'_> = env.new_string("NOT A NUMBER").expect("test string can be allocated");

		let err_result: std::result::Result<abos_jni::JValueOwned<'_>, abos_jni::errors::Error> = env
			.call_static_method(
				abos_jni::jni_str!("java/lang/Integer"),
				abos_jni::jni_str!("parseInt"),
				abos_jni::jni_sig!((str: JString) -> jint),
				&[(&jstring_not_numeric).into()],
			);
		assert!(matches!(err_result, Err(abos_jni::errors::Error::JavaException)));

		let res: std::result::Result<(), abos_jni::errors::Error> = if env.exception_check() {
			env.with_local_frame(16, |env| -> abos_jni::errors::Result<()> {
				let e: JThrowable<'_> = env
					.exception_occurred()
					.expect("Expected an exception after ExceptionCheck");
				env.exception_clear();

				classify_test_exception(env, &e)
			})
		} else {
			Ok(())
		};

		eprintln!("Caught exception: {res:?}");
		assert!(matches!(res, Err(Error::ParseFailed(_))));

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn classify_test_exception(env: &mut abos_jni::Env, e: &abos_jni::objects::JThrowable) -> abos_jni::errors::Result<()> {
	if let Some(_oom) = abos_jni::exceptions::JOutOfMemoryError::matches(env, e)? {
		Err(abos_jni::errors::Error::JniCall(JniError::NoMemory))
	} else if let Some(eformat) = abos_jni::exceptions::JNumberFormatException::matches(env, e)? {
		let msg: JString<'_> = eformat.as_throwable().get_message(env).unwrap_or_default();
		let msg: String = msg.to_string();
		Err(Error::ParseFailed(msg))
	} else {
		eprintln!("Unexpected exception: {e:?}");
		Err(Error::JniCall(JniError::Unknown))
	}
}
