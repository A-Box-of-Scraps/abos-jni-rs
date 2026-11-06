use super::*;
use assert_matches::assert_matches;

/// Transcodes text in an arbitrary Windows codepage into a Rust `String`. Used to test
/// round-tripping.
fn codepage_to_string_win32(
	codepage_string: impl AsRef<[u8]>,
	codepage: WCodepage,
	max_expected_utf16_len: WSize,
) -> io::Result<String> {
	let codepage_string_slice: &[u8] = codepage_string.as_ref();

	let codepage_string_slice_len: WSize = codepage_string_slice
		.len()
		.try_into()
		.expect("`codepage_string`'s length is too large to transcode with Win32");

	let mut buf: Vec<u16> = Vec::<u16>::with_capacity(
		max_expected_utf16_len
			.try_into()
			.expect("expected_utf16_len is negative or exceeds address space"),
	);

	// Safety: All of these pointers and lengths are valid and checked for overflow.
	let utf16_units_transcoded: i32 = unsafe {
		windows_sys::MultiByteToWideChar(
			codepage,
			0,
			codepage_string_slice.as_ptr(),
			codepage_string_slice_len,
			buf.as_mut_ptr(),
			max_expected_utf16_len,
		)
	};

	if utf16_units_transcoded == 0 {
		return Err(io::Error::last_os_error());
	}

	// Safety: `MultiByteToWideChar` claims to have initialized this many UTF-16 units.
	unsafe {
		buf.set_len(usize::try_from(utf16_units_transcoded).expect("successful Win32 UTF-16 length is positive"));
	}

	drop(codepage_string);

	let string: String = String::from_utf16(buf.as_slice()).expect("`MultiByteToWideChar` generated invalid UTF-16");

	Ok(string)
}

#[test]
fn test() {
	{
		let result: Cow<'_, CStr> =
			str_to_cstr_win32("Hello, world 😎".into(), windows_sys::CP_UTF8).expect("UTF-8 conversion succeeds");
		assert_eq!(result.to_bytes_with_nul(), b"Hello, world \xf0\x9f\x98\x8e\0");
		assert_matches!(result, Cow::Owned(_));
	}

	{
		let result: Cow<'_, CStr> =
			str_to_cstr_win32("Hello, world 😎\0".into(), windows_sys::CP_UTF8).expect("UTF-8 conversion succeeds");
		assert_eq!(result.to_bytes_with_nul(), b"Hello, world \xf0\x9f\x98\x8e\0");
	}

	{
		let result: JvmError =
			str_to_cstr_win32("Hello, world 😎".into(), 1252).expect_err("Windows-1252 cannot represent the emoji");
		let error_string: String =
			assert_matches!(result, JvmError::OptStringNotRepresentable { opt_string } => opt_string);
		assert_eq!(error_string, "Hello, world 😎");
	}

	{
		let result: Cow<'_, CStr> =
			str_to_cstr_win32("Hello, world™".into(), 1252).expect("Windows-1252 represents the trademark symbol");
		assert_eq!(result.to_bytes_with_nul(), b"Hello, world\x99\0");
		assert_matches!(result, Cow::Owned(_));
	}
}

// Note: We avoid `assert_eq!` here, because assertion failure will dump millions of
// characters to the console. Instead, here are some functions for handling errors without
// doing that.

#[track_caller]
fn check_and_clear_error_opt_string(expected_opt_string: &str, error: &mut JvmError) {
	if let Some(actual_opt_string) = error.opt_string_mut() {
		assert!(
			actual_opt_string == expected_opt_string,
			"opt_string was mangled in moving it to an error"
		);

		*actual_opt_string = String::new();
	}
}

#[track_caller]
fn expect_success(expected_opt_string: &str, result: Result<Cow<'static, CStr>, JvmError>) -> Cow<'static, CStr> {
	match result {
		Ok(ok) => ok,
		Err(mut error) => {
			check_and_clear_error_opt_string(expected_opt_string, &mut error);
			panic!("unexpected transcoding failure: {error}")
		}
	}
}

#[track_caller]
fn expect_successful_roundtrip(
	expected_opt_string: &str,
	result: Result<Cow<'static, CStr>, JvmError>,
) -> Cow<'static, CStr> {
	let string: Cow<'_, CStr> = expect_success(expected_opt_string, result);
	assert!(
		expected_opt_string.as_bytes() == string.to_bytes(),
		"opt_string was transcoded successfully but mangled"
	);
	string
}

#[track_caller]
fn expect_opt_string_too_long(expected_opt_string: &str, result: Result<Cow<'static, CStr>, JvmError>) {
	let mut error: JvmError = match result {
		Err(err) => err,
		Ok(ok) => {
			assert!(
				expected_opt_string.as_bytes() == ok.to_bytes(),
				"transcoding unexpectedly succeeded and resulted in mangled output"
			);
			panic!("transcoding unexpectedly succeeded")
		}
	};

	check_and_clear_error_opt_string(expected_opt_string, &mut error);

	assert_matches!(error, JvmError::OptStringTooLong { .. });
}

#[test]
fn test_overflow() {
	{
		// Try transcoding a plain ASCII string.

		// First, allocate enough space to completely fill the maximum allowed length, plus one
		// more.
		//eprintln!("Allocating & filling ASCII");
		let string: Vec<u8> = vec![b'H'; MAX_INPUT_LEN + 1];

		//eprintln!("Checking UTF-8 correctness");
		let mut string: String = String::from_utf8(string).expect("ASCII fixture is valid UTF-8");

		// This string is currently one character too long to transcode, so there should be an
		// overflow error.
		//eprintln!("Transcoding ASCII string that's too long");
		expect_opt_string_too_long(&string, str_to_cstr_win32(string.as_str().into(), windows_sys::CP_UTF8));

		// But if we remove one character…
		assert_eq!(string.pop(), Some('H'));

		// …then it should transcode fine.
		//eprintln!("Transcoding ASCII string that's not too long");
		expect_successful_roundtrip(&string, str_to_cstr_win32(string.as_str().into(), windows_sys::CP_UTF8));
	}

	{
		// Try transcoding a non-ASCII string.

		// U+07FF is the highest code point that can be represnted in UTF-8 with only two bytes, so
		// we'll use that. The UTF-8 encoding is `df bf`. We fill it this way because it's much
		// faster than the naïve character-by-character approach (at least unless some future Rust
		// compiler performs this optimization on its own, but 1.66 doesn't).
		//eprintln!("Allocating & filling non-ASCII for UTF-8 and UTF-7");
		let string_byte_pairs: Vec<u16> = vec![u16::from_be(0xdfbf); MAX_INPUT_LEN / 2];

		//eprintln!("Checking UTF-8 correctness");
		let string: &str = std::str::from_utf8(bytemuck::cast_slice(string_byte_pairs.as_slice()))
			.expect("fixture contains valid UTF-8 pairs");

		// Again, the string should transcode without overflow.
		//eprintln!("Transcoding non-ASCII to UTF-8");
		expect_successful_roundtrip(string, str_to_cstr_win32(string.into(), windows_sys::CP_UTF8));

		// This should work even with UTF-7. This is the real reason we're using U+07FF: we need
		// to check that the highest code point that fits under the limit will not overflow even
		// with the worst-case code page.
		{
			//eprintln!("Transcoding non-ASCII to UTF-7");
			let result: Cow<'_, CStr> = expect_success(string, str_to_cstr_win32(string.into(), windows_sys::CP_UTF7));

			// *And* it should roundtrip back to UTF-8.
			//eprintln!("Transcoding UTF-7 back to UTF-8");
			let result: String = codepage_to_string_win32(
				result.to_bytes(),
				windows_sys::CP_UTF7,
				(string.len() / 2)
					.try_into()
					.expect("fixture length fits Win32 integer"),
			)
			.expect("UTF-7 roundtrip conversion succeeds");

			assert!(result == string, "didn't roundtrip via UTF-7");
		}
	}

	{
		// Try transcoding to Windows-1252. This is the slowest part of the test
		// (`WideCharToMultiByte` is very slow at this, for some reason), so it's done last.
		//eprintln!("Allocating & filling non-ASCII for Windows-1252");
		let string_byte_pairs: Vec<u16> = vec![u16::from_be(0xc2ae); MAX_INPUT_LEN / 2];

		//eprintln!("Checking UTF-8 correctness");
		let string: &str = std::str::from_utf8(bytemuck::cast_slice(string_byte_pairs.as_slice()))
			.expect("fixture contains valid UTF-8 pairs");

		//eprintln!("Transcoding non-ASCII to Windows-1252");
		let result: Cow<'_, CStr> = expect_success(string, str_to_cstr_win32(string.into(), 1252));

		//eprintln!("Checking Windows-1252 for correctness");
		assert!(
			result.to_bytes().iter().all(|byte| *byte == 0xae),
			"string didn't transcode to Windows-1252 properly"
		);
	}
}

impl JvmError {
	const fn opt_string_mut(&mut self) -> Option<&mut String> {
		match self {
			Self::TooManyOptions(_) => None,
			Self::NullOptString(opt_string)
			| Self::OptStringTooLong { opt_string, .. }
			| Self::OptStringNotRepresentable { opt_string, .. }
			| Self::OptStringTranscodeFailure { opt_string, .. } => Some(opt_string),
		}
	}
}
