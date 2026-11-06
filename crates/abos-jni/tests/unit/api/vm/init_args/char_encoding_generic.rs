use super::{CStr, Cow, JvmError, utf8_to_cstr};

#[test]
fn test() {
	use assert_matches::assert_matches;

	{
		let result: Cow<'_, CStr> = utf8_to_cstr("Hello, world 😎".into()).expect("UTF-8 string can be NUL-terminated");
		assert_eq!(result.to_bytes_with_nul(), b"Hello, world \xf0\x9f\x98\x8e\0");
		assert_matches!(result, Cow::Owned(_));
	}

	{
		let result: Cow<'_, CStr> = utf8_to_cstr("Hello, world 😎\0".into()).expect("trailing NUL is accepted");
		assert_eq!(result.to_bytes_with_nul(), b"Hello, world \xf0\x9f\x98\x8e\0");
		assert_matches!(result, Cow::Borrowed(_));
	}

	{
		let result: JvmError = utf8_to_cstr("Hello,\0world".into()).expect_err("interior NUL is rejected");
		let error_string: String = assert_matches!(result, JvmError::NullOptString(string) => string);
		assert_eq!(error_string, "Hello,\0world");
	}
}
