use std::ffi::CStr;

use super::{JNIString, normalize};
use crate::strings::JNIStr;

#[test]
fn strict_mutf8_needs_no_copy() {
	for input in [
		c"",
		c"ASCII",
		c"é日",
		c"\xC0\x80",
		c"\xED\xA0\xBD\xED\xB8\x80",
		c"\xED\xA0\x80\xC0\x80\xED\xB0\x80",
	] {
		assert!(JNIStr::from_cstr(input).is_some());
		// SAFETY: The inputs were validated as strict MUTF-8 above.
		assert!(unsafe { normalize(input) }.is_none(), "input: {input:?}");
	}
}

#[test]
fn four_byte_sequences_preserve_nuls_and_other_characters() {
	let cases: &[(&CStr, &str)] = &[
		(c"😀", "😀"),
		(c"\xC0\x80😀", "\0😀"),
		(c"😀\xC0\x80", "😀\0"),
		(c"é\xC0\x80😀日\xC0\x80😀", "é\0😀日\0😀"),
		(c"😀\xED\xA0\xBD\xED\xB8\x80😀", "😀😀😀"),
		(c"\xF0\x90\x80\x80\xF4\x8F\xBF\xBF", "\u{10000}\u{10FFFF}"),
	];
	for &(input, expected) in cases {
		// SAFETY: The literals contain valid MUTF-8 units and four-byte UTF-8 characters.
		let normalized: JNIString = unsafe { normalize(input) }.expect("contains a four-byte sequence");
		assert_eq!(normalized, JNIString::new(expected), "input: {input:?}");
		assert_eq!(normalized.to_str(), expected);
		assert!(JNIStr::from_cstr(normalized.as_cstr()).is_some());
	}
}

#[test]
fn unpaired_surrogates_are_preserved_as_code_units() {
	let input: &CStr = c"\xED\xA0\x80😀\xC0\x80\xED\xB0\x80😀\xED\xA0\x80";
	let expected: &CStr =
		c"\xED\xA0\x80\xED\xA0\xBD\xED\xB8\x80\xC0\x80\xED\xB0\x80\xED\xA0\xBD\xED\xB8\x80\xED\xA0\x80";
	// SAFETY: The input contains complete MUTF-8 units and valid four-byte UTF-8 characters.
	let normalized: JNIString = unsafe { normalize(input) }.expect("contains supplementary characters");
	assert_eq!(normalized.as_cstr(), expected);
	assert_eq!(normalized.to_str(), "\u{FFFD}😀\0\u{FFFD}😀\u{FFFD}");
}

#[test]
fn supplementary_characters_match_the_standard_mutf8_encoder() {
	for value in (0x0001_0000..=0x0010_FFFF).step_by(997) {
		let character: char = char::from_u32(value).expect("supplementary scalar value");
		let input: std::ffi::CString = std::ffi::CString::new(character.to_string()).expect("non-NUL scalar");
		// SAFETY: Every supplementary scalar is encoded as valid four-byte UTF-8.
		let normalized: JNIString = unsafe { normalize(&input) }.expect("four-byte sequence");
		assert_eq!(normalized, JNIString::new(character.to_string()));
	}
}
