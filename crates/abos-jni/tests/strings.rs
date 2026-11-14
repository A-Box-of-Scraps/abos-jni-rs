#![allow(missing_docs, reason = "Integration tests are not library API.")]

use abos_jni::strings::JNIStr;

#[test]
fn lossy_conversion_preserves_valid_characters() {
	// NUL and U+1F600 occur on both sides of an unpaired high surrogate.
	let input: &JNIStr =
		JNIStr::from_cstr(c"before:\xC0\x80\xED\xA0\xBD\xED\xB8\x80\xED\xA0\x80after:\xC0\x80\xED\xA0\xBD\xED\xB8\x80")
			.expect("unpaired UTF-16 surrogates are valid in Java strings");
	assert_eq!(input.to_str(), "before:\0😀\u{FFFD}after:\0😀");
}

#[test]
fn lossy_conversion_replaces_each_unpaired_surrogate_once() {
	let cases: &[(&std::ffi::CStr, &str)] = &[
		(c"\xED\xA0\x80", "\u{FFFD}"),
		(c"\xED\xAF\xBF", "\u{FFFD}"),
		(c"\xED\xB0\x80", "\u{FFFD}"),
		(c"\xED\xBF\xBF", "\u{FFFD}"),
		(c"\xED\xA0\x80\xED\xA0\x80", "\u{FFFD}\u{FFFD}"),
		(c"\xED\xB0\x80\xED\xB0\x80", "\u{FFFD}\u{FFFD}"),
		(c"\xED\xB0\x80\xED\xA0\x80", "\u{FFFD}\u{FFFD}"),
		(c"\xED\xA0\x80\xED\xA0\xBD\xED\xB8\x80", "\u{FFFD}😀"),
		(c"\xED\xA0\xBD\xED\xB8\x80\xED\xB0\x80", "😀\u{FFFD}"),
		(c"\xED\xA0\x80\xC0\x80\xED\xB0\x80", "\u{FFFD}\0\u{FFFD}"),
		(c"é\xED\xA0\x80日\xED\xB0\x80", "é\u{FFFD}日\u{FFFD}"),
	];
	for &(encoded, expected) in cases {
		let input: &JNIStr = JNIStr::from_cstr(encoded).expect("valid MUTF-8 code units");
		let decoded: std::borrow::Cow<'_, str> = input.to_str();
		assert_eq!(decoded, expected, "input: {encoded:?}");
		assert!(matches!(decoded, std::borrow::Cow::Owned(_)));
		assert_eq!(input.to_string(), expected);
		let owned: String = input.to_owned().into();
		assert_eq!(owned, expected);
	}
}

#[test]
fn unchanged_utf8_is_borrowed() {
	for text in [
		"",
		"ASCII",
		"é日",
		"\u{007F}\u{0080}\u{07FF}\u{0800}\u{D7FF}\u{E000}\u{FFFF}",
	] {
		let input: abos_jni::strings::JNIString = abos_jni::strings::JNIString::new(text);
		let decoded: std::borrow::Cow<'_, str> = input.to_str();
		assert_eq!(decoded, text);
		assert!(matches!(decoded, std::borrow::Cow::Borrowed(_)));
	}
}

#[test]
fn valid_nuls_and_supplementary_characters_are_decoded() {
	for text in ["\0", "😀", "\u{10000}\u{10FFFF}", "é\0😀日"] {
		let input: abos_jni::strings::JNIString = abos_jni::strings::JNIString::new(text);
		let decoded: std::borrow::Cow<'_, str> = input.to_str();
		assert_eq!(decoded, text);
		assert!(matches!(decoded, std::borrow::Cow::Owned(_)));
	}
}

#[test]
fn strict_mutf8_constructors_reject_four_byte_utf8() {
	for input in [c"😀", c"\xC0\x80😀", c"😀\xC0\x80", c"\xF4\x8F\xBF\xBF"] {
		assert!(JNIStr::from_cstr(input).is_none(), "input: {input:?}");
	}
}
