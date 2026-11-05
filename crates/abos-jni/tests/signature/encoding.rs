use super::*;

#[test]
fn test_mutf8_encoding_basic_ascii() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr};

	// Basic ASCII should be the same in MUTF-8
	let sig = jni_sig!((a: java.lang.String) -> void);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/String;)V");

	let cstr_sig = jni_sig_cstr!((a: java.lang.String) -> void);
	assert_eq!(cstr_sig.to_bytes(), b"(Ljava/lang/String;)V");

	let jstr_sig = jni_sig_jstr!((a: java.lang.String) -> void);
	assert_eq!(jstr_sig.to_bytes(), b"(Ljava/lang/String;)V");
}

#[test]
fn test_mutf8_encoding_unicode_emoji() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr};

	// Test Unicode emoji in class name - requires MUTF-8 encoding
	// The emoji 😀 (U+1F600) in MUTF-8 is encoded as surrogate pairs:
	// U+1F600 -> UTF-16 surrogate pair: D83D DE00
	// In MUTF-8: ED A0 BD ED B8 80
	let sig = jni_sig!((a: "unicode.Type😀") -> bool);
	let expected = b"(Lunicode/Type\xed\xa0\xbd\xed\xb8\x80;)Z";
	assert_eq!(sig.sig().to_bytes(), expected);

	let cstr_sig = jni_sig_cstr!((a: "unicode.Type😀") -> bool);
	assert_eq!(cstr_sig.to_bytes(), expected);

	let jstr_sig = jni_sig_jstr!((a: "unicode.Type😀") -> bool);
	assert_eq!(jstr_sig.to_bytes(), expected);
}

#[test]
fn test_mutf8_encoding_unicode_field() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr};

	// Test Unicode emoji in field signature
	let sig = jni_sig!("unicode.Field😀");
	let expected = b"Lunicode/Field\xed\xa0\xbd\xed\xb8\x80;";
	assert_eq!(sig.sig().to_bytes(), expected);

	let cstr_sig = jni_sig_cstr!("unicode.Field😀");
	assert_eq!(cstr_sig.to_bytes(), expected);

	let jstr_sig = jni_sig_jstr!("unicode.Field😀");
	assert_eq!(jstr_sig.to_bytes(), expected);
}

#[test]
fn test_mutf8_encoding_unicode_various_chars() {
	use abos_jni_macros::jni_sig_cstr;

	// Test various Unicode characters
	// Japanese: こんにちは (Konnichiwa)
	let sig_jp = jni_sig_cstr!("jp.こんにちは");
	// こ (U+3053) -> E3 81 93
	// ん (U+3093) -> E3 82 93
	// に (U+306B) -> E3 81 AB
	// ち (U+3061) -> E3 81 A1
	// は (U+306F) -> E3 81 AF
	assert_eq!(
		sig_jp.to_bytes(),
		b"Ljp/\xe3\x81\x93\xe3\x82\x93\xe3\x81\xab\xe3\x81\xa1\xe3\x81\xaf;"
	);

	// Emoji combination: 👍 (U+1F44D)
	// U+1F44D -> UTF-16 surrogate pair: D83D DC4D
	// In MUTF-8: ED A0 BD ED B1 8D
	let sig_emoji = jni_sig_cstr!("emoji.👍");
	assert_eq!(sig_emoji.to_bytes(), b"Lemoji/\xed\xa0\xbd\xed\xb1\x8d;");
}

#[test]
fn test_all_three_macros_produce_same_encoding() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr};

	// All three macros should produce the same byte sequence for the signature
	let sig = jni_sig!((a: jint, b: java.lang.String) -> java.lang.Object);
	let cstr_sig = jni_sig_cstr!((a: jint, b: java.lang.String) -> java.lang.Object);
	let jstr_sig = jni_sig_jstr!((a: jint, b: java.lang.String) -> java.lang.Object);

	assert_eq!(sig.sig().to_bytes(), cstr_sig.to_bytes());
	assert_eq!(sig.sig().to_bytes(), jstr_sig.to_bytes());
	assert_eq!(sig.sig().to_bytes(), b"(ILjava/lang/String;)Ljava/lang/Object;");
}

#[test]
fn test_const_evaluation_all_macros() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr, jni_sig_str};
	use std::ffi::CStr;

	// Test that all macros work in const contexts
	const STR_SIG: &str = jni_sig_str!((a: jint) -> void);
	const CSTR_SIG: &CStr = jni_sig_cstr!((a: jint) -> void);
	const JSTR_SIG: &JNIStr = jni_sig_jstr!((a: jint) -> void);

	assert_eq!(STR_SIG, "(I)V");
	assert_eq!(CSTR_SIG.to_bytes(), b"(I)V");
	assert_eq!(JSTR_SIG.to_bytes(), b"(I)V");
}

// Tests for jni_sig_str! macro

#[test]
fn test_jni_sig_str_basic() {
	use abos_jni_macros::jni_sig_str;

	const SIG: &str = jni_sig_str!((a: jint, b: jboolean) -> void);
	assert_eq!(SIG, "(IZ)V");
}

#[test]
fn test_jni_sig_str_object() {
	use abos_jni_macros::jni_sig_str;

	const SIG: &str = jni_sig_str!((a: java.lang.String) -> java.lang.Object);
	assert_eq!(SIG, "(Ljava/lang/String;)Ljava/lang/Object;");
}

#[test]
fn test_jni_sig_str_field() {
	use abos_jni_macros::jni_sig_str;

	const FIELD_SIG: &str = jni_sig_str!(java.lang.String);
	assert_eq!(FIELD_SIG, "Ljava/lang/String;");
}

#[test]
fn test_jni_sig_str_array() {
	use abos_jni_macros::jni_sig_str;

	const SIG: &str = jni_sig_str!(([jint], [java.lang.String]) -> [[jint]]);
	assert_eq!(SIG, "([I[Ljava/lang/String;)[[I");
}

#[test]
fn test_jni_sig_str_with_type_map() {
	use abos_jni_macros::jni_sig_str;

	const SIG: &str = jni_sig_str!(
		(a: CustomType) -> void,
		type_map = {
			CustomType => java.lang.Custom,
		}
	);
	assert_eq!(SIG, "(Ljava/lang/Custom;)V");
}

// Tests for jni_sig_cstr! macro

#[test]
fn test_jni_sig_cstr_basic() {
	use abos_jni_macros::jni_sig_cstr;
	use std::ffi::CStr;

	const SIG: &CStr = jni_sig_cstr!((a: jint, b: jboolean) -> void);
	assert_eq!(SIG.to_bytes(), b"(IZ)V");
}

#[test]
fn test_jni_sig_cstr_object() {
	use abos_jni_macros::jni_sig_cstr;
	use std::ffi::CStr;

	const SIG: &CStr = jni_sig_cstr!((a: java.lang.String) -> java.lang.Object);
	assert_eq!(SIG.to_bytes(), b"(Ljava/lang/String;)Ljava/lang/Object;");
}

#[test]
fn test_jni_sig_cstr_field() {
	use abos_jni_macros::jni_sig_cstr;
	use std::ffi::CStr;

	const FIELD_SIG: &CStr = jni_sig_cstr!(java.lang.String);
	assert_eq!(FIELD_SIG.to_bytes(), b"Ljava/lang/String;");
}

#[test]
fn test_jni_sig_cstr_array() {
	use abos_jni_macros::jni_sig_cstr;
	use std::ffi::CStr;

	const SIG: &CStr = jni_sig_cstr!(([jint], [java.lang.String]) -> [[jint]]);
	assert_eq!(SIG.to_bytes(), b"([I[Ljava/lang/String;)[[I");
}

// Tests for jni_sig_jstr! macro

#[test]
fn test_jni_sig_jstr_basic() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_sig_jstr;

	const SIG: &JNIStr = jni_sig_jstr!((a: jint, b: jboolean) -> void);
	assert_eq!(SIG.to_bytes(), b"(IZ)V");
}

#[test]
fn test_jni_sig_jstr_object() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_sig_jstr;

	const SIG: &JNIStr = jni_sig_jstr!((a: java.lang.String) -> java.lang.Object);
	assert_eq!(SIG.to_bytes(), b"(Ljava/lang/String;)Ljava/lang/Object;");
}

#[test]
fn test_jni_sig_jstr_field() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_sig_jstr;

	const FIELD_SIG: &JNIStr = jni_sig_jstr!(java.lang.String);
	assert_eq!(FIELD_SIG.to_bytes(), b"Ljava/lang/String;");
}

#[test]
fn test_jni_sig_jstr_array() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_sig_jstr;

	const SIG: &JNIStr = jni_sig_jstr!(([jint], [java.lang.String]) -> [[jint]]);
	assert_eq!(SIG.to_bytes(), b"([I[Ljava/lang/String;)[[I");
}

#[test]
fn test_jni_sig_jstr_with_type_map() {
	use abos_jni::strings::JNIStr;
	use abos_jni_macros::jni_sig_jstr;

	const SIG: &JNIStr = jni_sig_jstr!(
		(a: CustomType) -> void,
		type_map = {
			CustomType => java.lang.Custom,
		}
	);
	assert_eq!(SIG.to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_jni_sig_str_produces_utf8_string() {
	use abos_jni_macros::jni_sig_str;

	// jni_sig_str! should produce a regular UTF-8 string
	const SIG: &str = jni_sig_str!((a: jint, b: java.lang.String) -> void);
	assert_eq!(SIG, "(ILjava/lang/String;)V");
	assert_eq!(SIG.as_bytes(), b"(ILjava/lang/String;)V");
}

#[test]
fn test_field_signatures_all_variants() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr, jni_sig_str};

	// Test that field signatures work with all macro variants
	let field_sig = jni_sig!(java.util.List);
	assert_eq!(field_sig.sig().to_bytes(), b"Ljava/util/List;");

	let cstr_field = jni_sig_cstr!(java.util.List);
	assert_eq!(cstr_field.to_bytes(), b"Ljava/util/List;");

	let jstr_field = jni_sig_jstr!(java.util.List);
	assert_eq!(jstr_field.to_bytes(), b"Ljava/util/List;");

	const STR_FIELD: &str = jni_sig_str!(java.util.List);
	assert_eq!(STR_FIELD, "Ljava/util/List;");
}

#[test]
fn test_raw_signature_all_variants() {
	use abos_jni_macros::{jni_sig_cstr, jni_sig_jstr, jni_sig_str};

	// Test raw signatures with all macro variants
	let sig = jni_sig!("(ILjava/lang/String;)V");
	assert_eq!(sig.sig().to_bytes(), b"(ILjava/lang/String;)V");

	let cstr_sig = jni_sig_cstr!("(ILjava/lang/String;)V");
	assert_eq!(cstr_sig.to_bytes(), b"(ILjava/lang/String;)V");

	let jstr_sig = jni_sig_jstr!("(ILjava/lang/String;)V");
	assert_eq!(jstr_sig.to_bytes(), b"(ILjava/lang/String;)V");

	const STR_SIG: &str = jni_sig_str!("(ILjava/lang/String;)V");
	assert_eq!(STR_SIG, "(ILjava/lang/String;)V");
}

// Tests for jni_cstr! and jni_str! macros
