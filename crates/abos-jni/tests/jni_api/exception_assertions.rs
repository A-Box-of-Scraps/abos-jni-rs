use super::*;

// Shortcut to `assert_pending_java_exception_detailed()` without checking for expected  type and
// message of exception.
pub(super) fn assert_pending_java_exception(env: &mut Env) {
	assert_pending_java_exception_detailed(env, None, None)
}

// Helper method that asserts there is a pending Java exception of `expected_type` with
// `expected_message` and clears it if any.
pub(super) fn assert_pending_java_exception_detailed(
	env: &mut Env,
	expected_type: Option<&JNIStr>,
	expected_message: Option<&JNIStr>,
) {
	assert!(env.exception_check());
	let exception = env.exception_occurred().expect("Unable to get exception");
	env.exception_clear();

	if let Some(expected_type) = expected_type {
		assert_exception_type(env, &exception, expected_type);
	}

	if let Some(expected_message) = expected_message {
		assert_exception_message(env, &exception, expected_message);
	}
}

// Asserts that exception is of `expected_type` type.
pub(super) fn assert_exception_type(env: &mut Env, exception: &JThrowable, expected_type: &JNIStr) {
	assert!(env.is_instance_of(exception, expected_type).unwrap());
}

// Asserts that exception's message is `expected_message`.
pub(super) fn assert_exception_message(env: &mut Env, exception: &JThrowable, expected_message: &JNIStr) {
	let message = exception.get_message(env).unwrap();
	let msg_rust: JNIString = message.mutf8_chars(env).unwrap().into();
	assert_eq!(msg_rust, expected_message);
}
