#[cfg(doc)]
use crate::{Env, JValue, JValueOwned};

use super::{CharToJavaError, CharTryFromError, DecodeUtf16Error, TryFrom, jchar, jint};

/// Converts a Java `char` to a Rust `char`, if possible. (Error-prone; see warning.)
///
/// **Warning:** Converting a single Java `char` to a Rust `char` is not recommended. This can only
/// succeed for code points up to U+FFFF. Code points above that are represented in Java as *two*
/// `char`s, each representing one half of a UTF-16 [surrogate pair], which this function cannot
/// handle. If possible, use one of these alternatives instead:
///
/// * Use Java `int`s containing UTF-32. You can encode a Java `String` as a sequence of UTF-32
///   `int`s using its [`codePoints`] method, then use [`char_from_java_int`] to convert each one to
///   a Rust `char`.
///
/// * Convert a Java `String` using [`Env::get_string`].
///
/// * Convert multiple Java `char`s at a time (such as in a Java `char[]` array) using
///   [`char::decode_utf16`] (which this function is a simple wrapper around). That will properly
///   convert any surrogate pairs among the Java `char`s.
///
/// # See Also
///
/// * [`JValue::c_char`], a wrapper around this function that unwraps [`JValue::Char`]
/// * [`JValueOwned::c_char`], a wrapper around this function that unwraps [`JValueOwned::Char`]
/// * [`char_to_java`], the opposite of this function
/// * [`char_from_java_int`], a UTF-32 alternative to this function that is unlikely to fail
///
/// # Errors
///
/// This function returns an error if the provided Java `char` is part of a UTF-16 surrogate pair,
/// which cannot be converted to a Rust `char` by itself.
///
/// [`codePoints`]:
///     https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/String.html#codePoints()
/// [surrogate pair]: https://en.wikipedia.org/wiki/Surrogate_pair
pub fn char_from_java(char: jchar) -> std::result::Result<char, DecodeUtf16Error> {
	char::decode_utf16([char])
		.next()
		.expect("one UTF-16 code unit produces one decoding result")
}

/// Converts a Rust `char` to a Java `char`, if possible. (Error-prone; see warning.)
///
/// **Warning:** Converting a Rust `char` to a single Java `char` is not recommended. This can only
/// succeed for code points up to U+FFFF. Code points above that are represented in Java as *two*
/// `char`s, each representing one half of a UTF-16 [surrogate pair], which this function cannot
/// handle. If possible, use one of these alternatives instead:
///
/// * Use Java `int`s containing UTF-32. You can convert a Rust `char` to a Java `int` containing
///   UTF-32 using [`char_to_java_int`], which never fails.
///
/// * Convert a Rust [`str`] to a Java `String` using [`Env::new_string`].
///
/// * Convert a Rust `char` to multiple Java `char`s at a time using [`char::encode_utf16`] (which
///   this function is a wrapper around). That will properly generate surrogate pairs as needed.
///
/// # See Also
///
/// * [`JValue`] and [`JValueOwned`]'s implementation of `TryFrom<char>`, a wrapper for this function that produces
///   [`JValue::Char`] and [`JValueOwned::Char`]
/// * [`char_from_java`], the opposite of this function
/// * [`char_to_java_int`], a UTF-32 alternative to this function that never fails
///
/// # Errors
///
/// This function returns an error if the provided `char` cannot be represented in UTF-16 without a
/// surrogate pair, and therefore cannot be converted to a single Java `char`.
///
/// [surrogate pair]: https://en.wikipedia.org/wiki/Surrogate_pair
pub const fn char_to_java(char: char) -> std::result::Result<jchar, CharToJavaError> {
	if char.len_utf16() != 1 {
		return Err(CharToJavaError { char });
	}

	let mut buf = [0u16; 1];
	let buf: &mut [u16] = char.encode_utf16(&mut buf);
	Ok(buf[0])
}

// jshort
/// Converts a Rust `char` to a Java `int`.
///
/// This is the form expected or produced by certain Java APIs that process UTF-32 units, such as [`String.codePointAt`].
///
/// As discussed in [`char_to_java`], Rust `char` cannot always be converted to Java `char`, but can always be converted to Java `int`. This is the recommended way to pass a Rust `char` to Java code.
///
/// # See Also
///
/// * [`JValue::int_from_char`], a wrapper for this function that returns [`JValue::Int`]
/// * [`char_from_java_int`], the opposite of this function
/// * [`char_to_java`], an alternative to this function that converts to Java `char` but is likely to fail
///
/// [`String.codePointAt`]: https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/String.html#codePointAt(int)
#[must_use]
pub fn char_to_java_int(char: char) -> jint {
	u32::from(char) as jint
}

/// Converts a Java `int` to a Rust `char`.
///
/// This is the form expected or produced by certain Java APIs that process UTF-32 units, such as [`String.codePointAt`].
///
/// As discussed in [`char_from_java`], Rust `char` cannot always be converted from Java `char`, but can always be converted from Java `int` (provided that the `int` contains a valid UTF-32 unit). This is the recommended way to receive a Rust `char` from Java code.
///
/// # See Also
///
/// * [`JValue::i_char`], a wrapper for this function that unwraps [`JValue::Int`]
/// * [`JValueOwned::i_char`], a wrapper for this function that unwraps [`JValueOwned::Int`]
/// * [`char_to_java_int`], the opposite of this function
/// * [`char_from_java`], an alternative to this function that converts from Java `char` but is likely to fail
///
/// # Errors
///
/// Returns an error if the Java `int` doesn't represent a valid UTF-32 unit.
///
/// [`String.codePointAt`]: https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/lang/String.html#codePointAt(int)
pub fn char_from_java_int(jint: jint) -> std::result::Result<char, CharTryFromError> {
	char::try_from(jint as u32)
}

// jlong
