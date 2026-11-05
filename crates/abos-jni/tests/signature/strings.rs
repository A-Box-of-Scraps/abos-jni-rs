#[test]
fn test_jni_cstr_basic() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	const CLASS_NAME: &CStr = jni_cstr!("java.lang.String");
	assert_eq!(CLASS_NAME.to_bytes(), b"java.lang.String");
}

#[test]
fn test_jni_cstr_unicode_emoji() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	// Test Unicode emoji (above U+FFFF) - requires MUTF-8 encoding with surrogate pairs
	const EMOJI_CLASS: &CStr = jni_cstr!("unicode.Type😀");
	// The emoji 😀 (U+1F600) in MUTF-8 is encoded as surrogate pairs:
	// U+1F600 -> UTF-16 surrogate pair: D83D DE00
	// In MUTF-8: ED A0 BD ED B8 80
	assert_eq!(EMOJI_CLASS.to_bytes(), b"unicode.Type\xed\xa0\xbd\xed\xb8\x80");

	// Verify roundtrip decode
	let decoded = simd_cesu8::mutf8::decode(EMOJI_CLASS.to_bytes()).expect("Failed to decode MUTF-8");
	assert_eq!(decoded, "unicode.Type😀");
}

#[test]
fn test_jni_str_basic() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	const CLASS_NAME: &JNIStr = jni_str!("java.lang.String");
	assert_eq!(CLASS_NAME.to_bytes(), b"java.lang.String");
}

#[test]
fn test_jni_str_unicode_emoji() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	// Test Unicode emoji (above U+FFFF) - requires MUTF-8 encoding with surrogate pairs
	const EMOJI_CLASS: &JNIStr = jni_str!("unicode.Type😀");
	assert_eq!(EMOJI_CLASS.to_bytes(), b"unicode.Type\xed\xa0\xbd\xed\xb8\x80");

	// Verify roundtrip decode
	let decoded = simd_cesu8::mutf8::decode(EMOJI_CLASS.to_bytes()).expect("Failed to decode MUTF-8");
	assert_eq!(decoded, "unicode.Type😀");
}

#[test]
fn test_jni_cstr_and_jni_str_produce_same_encoding() {
	use abos_jni_macros::{jni_cstr, jni_str};

	// Test that jni_cstr! and jni_str! produce the same byte encoding
	const CSTR: &std::ffi::CStr = jni_cstr!("java.util.ArrayList");
	const JSTR: &abos_jni::strings::JNIStr = jni_str!("java.util.ArrayList");

	assert_eq!(CSTR.to_bytes(), JSTR.to_bytes());
	assert_eq!(CSTR.to_bytes(), b"java.util.ArrayList");
}

#[test]
fn test_jni_cstr_const_evaluation() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	// Verify that jni_cstr! works in const contexts
	const PATH_SEP: &CStr = jni_cstr!("/");
	const PACKAGE: &CStr = jni_cstr!("com.example");
	const NESTED: &CStr = jni_cstr!("com.example.nested.Class");

	assert_eq!(PATH_SEP.to_bytes(), b"/");
	assert_eq!(PACKAGE.to_bytes(), b"com.example");
	assert_eq!(NESTED.to_bytes(), b"com.example.nested.Class");
}

#[test]
fn test_jni_str_const_evaluation() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	// Verify that jni_str! works in const contexts
	const PATH_SEP: &JNIStr = jni_str!("/");
	const PACKAGE: &JNIStr = jni_str!("com.example");
	const NESTED: &JNIStr = jni_str!("com.example.nested.Class");

	assert_eq!(PATH_SEP.to_bytes(), b"/");
	assert_eq!(PACKAGE.to_bytes(), b"com.example");
	assert_eq!(NESTED.to_bytes(), b"com.example.nested.Class");
}

#[test]
fn test_jni_cstr_concat_multiple_literals() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	// Test concat-like behavior with multiple string literals
	const CONCATENATED: &CStr = jni_cstr!("java", ".", "lang", ".", "String");
	assert_eq!(CONCATENATED.to_bytes(), b"java.lang.String");

	const TWO_PART: &CStr = jni_cstr!("com.example", ".MyClass");
	assert_eq!(TWO_PART.to_bytes(), b"com.example.MyClass");

	const WITH_TRAILING_COMMA: &CStr = jni_cstr!("hello", " ", "world",);
	assert_eq!(WITH_TRAILING_COMMA.to_bytes(), b"hello world");
}

#[test]
fn test_jni_str_concat_multiple_literals() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	// Test concat-like behavior with multiple string literals
	const CONCATENATED: &JNIStr = jni_str!("java", ".", "lang", ".", "String");
	assert_eq!(CONCATENATED.to_bytes(), b"java.lang.String");

	const TWO_PART: &JNIStr = jni_str!("com.example", ".MyClass");
	assert_eq!(TWO_PART.to_bytes(), b"com.example.MyClass");

	const WITH_TRAILING_COMMA: &JNIStr = jni_str!("hello", " ", "world",);
	assert_eq!(WITH_TRAILING_COMMA.to_bytes(), b"hello world");
}

#[test]
fn test_jni_str_with_jni_crate_override() {
	use abos_jni_macros::jni_str;

	// Test that we can override the jni crate path
	// This should compile successfully with jni = abos_jni
	const WITH_JNI_OVERRIDE: &abos_jni::strings::JNIStr = jni_str!(jni = abos_jni, "test.Class");
	assert_eq!(WITH_JNI_OVERRIDE.to_bytes(), b"test.Class");

	// Test with multiple literals after jni override
	const MULTI_WITH_OVERRIDE: &abos_jni::strings::JNIStr = jni_str!(jni = abos_jni, "test", ".", "Class");
	assert_eq!(MULTI_WITH_OVERRIDE.to_bytes(), b"test.Class");
}

#[test]
fn test_jni_cstr_and_jni_str_concat_same_encoding() {
	use abos_jni_macros::{jni_cstr, jni_str};

	// Test that jni_cstr! and jni_str! produce the same byte encoding when concatenating
	const CSTR: &std::ffi::CStr = jni_cstr!("java", ".", "util", ".", "ArrayList");
	const JSTR: &abos_jni::strings::JNIStr = jni_str!("java", ".", "util", ".", "ArrayList");

	assert_eq!(CSTR.to_bytes(), JSTR.to_bytes());
	assert_eq!(CSTR.to_bytes(), b"java.util.ArrayList");
}

#[test]
fn test_jni_cstr_mixed_literal_types() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	// Test mixing different literal types
	const PORT: &CStr = jni_cstr!("localhost:", 8080);
	assert_eq!(PORT.to_bytes(), b"localhost:8080");

	const VERSION: &CStr = jni_cstr!("Version ", 1, '.', 2, '.', 3);
	assert_eq!(VERSION.to_bytes(), b"Version 1.2.3");

	const FLAG: &CStr = jni_cstr!("enabled=", true);
	assert_eq!(FLAG.to_bytes(), b"enabled=true");
}

#[test]
fn test_jni_str_mixed_literal_types() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	// Test mixing different literal types
	const PORT: &JNIStr = jni_str!("localhost:", 8080);
	assert_eq!(PORT.to_bytes(), b"localhost:8080");

	const VERSION: &JNIStr = jni_str!("v", 2, '.', 0);
	assert_eq!(VERSION.to_bytes(), b"v2.0");

	const BYTE_VAL: &JNIStr = jni_str!("byte=", b'A');
	assert_eq!(BYTE_VAL.to_bytes(), b"byte=65"); // ASCII value of 'A'
}

#[test]
fn test_jni_cstr_float_literal() {
	use abos_jni_macros::jni_cstr;
	use std::ffi::CStr;

	const PI: &CStr = jni_cstr!("pi=", 3.14159);
	assert_eq!(PI.to_bytes(), b"pi=3.14159");
}

#[test]
fn test_jni_str_cstr_literal() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_str;

	// CStr literals can be used if they're valid UTF-8
	const HELLO: &JNIStr = jni_str!(c"Hello", " ", c"World");
	assert_eq!(HELLO.to_bytes(), b"Hello World");
}
