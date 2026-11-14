use super::Error;

#[test]
fn caught_exception_display_includes_class_message_and_stack() {
	let error: Error = Error::CaughtJavaException {
		exception: crate::refs::Global::default(),
		name: "java.lang.IllegalStateException".to_owned(),
		msg: "operation failed".to_owned(),
		stack: "0: example.Test.run(Test.java:42)\n".to_owned(),
	};
	assert_eq!(
		error.to_string(),
		"Caught Java exception: java.lang.IllegalStateException: operation failed\nStack trace:\n0: example.Test.run(Test.java:42)\n"
	);
}

#[test]
fn caught_exception_display_marks_empty_messages() {
	let error: Error = Error::CaughtJavaException {
		exception: crate::refs::Global::default(),
		name: "java.lang.NullPointerException".to_owned(),
		msg: String::new(),
		stack: String::new(),
	};
	assert_eq!(
		error.to_string(),
		"Caught Java exception: java.lang.NullPointerException: <no message>\nStack trace:\n"
	);
}

#[test]
fn caught_exception_display_preserves_literal_messages() {
	for message in ["<NULL>", " ", "détail 日本語\nsecond line"] {
		let error: Error = Error::CaughtJavaException {
			exception: crate::refs::Global::default(),
			name: "java.lang.Exception".to_owned(),
			msg: message.to_owned(),
			stack: "none".to_owned(),
		};
		assert_eq!(
			error.to_string(),
			format!("Caught Java exception: java.lang.Exception: {message}\nStack trace:\nnone")
		);
	}
}
