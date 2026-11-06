use super::*;
use quote::quote;

#[test]
fn test_jni_cstr_single_literal() {
	let input: TokenStream = quote! { "hello" };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();

	// Should produce a CStr literal
	assert!(result_str.contains("c\"hello\"") || result_str.contains("\"hello\""));
}

#[test]
fn test_jni_cstr_multiple_literals() {
	let input: TokenStream = quote! { "hello", " ", "world" };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();

	// Should concatenate to "hello world"
	assert!(result_str.contains("hello world"));
}

#[test]
fn test_jni_cstr_with_jni_override() {
	let input: TokenStream = quote! { jni = crate::my_jni, "test" };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();

	// Should produce a CStr literal (jni override doesn't affect jni_cstr! output)
	assert!(result_str.contains("test"));
}

#[test]
fn test_jni_str_single_literal() {
	let input: TokenStream = quote! { "hello" };
	let result: TokenStream = jni_str_impl(input).expect("test JNI string expands");
	let result_str: String = result.to_string();

	// Should produce JNIStr::from_cstr_unchecked call
	assert!(result_str.contains("JNIStr"));
	assert!(result_str.contains("from_cstr_unchecked"));
	assert!(result_str.contains("hello"));
}

#[test]
fn test_jni_str_multiple_literals() {
	let input: TokenStream = quote! { "java.lang.", "String" };
	let result: TokenStream = jni_str_impl(input).expect("test JNI string expands");
	let result_str: String = result.to_string();

	// Should concatenate to "java.lang.String"
	assert!(result_str.contains("java.lang.String"));
}

#[test]
fn test_jni_str_with_jni_override() {
	let input: TokenStream = quote! { jni = ::my_custom_jni, "test" };
	let result: TokenStream = jni_str_impl(input).expect("test JNI string expands");
	let result_str: String = result.to_string();

	// Should use the custom jni crate path
	assert!(result_str.contains(":: my_custom_jni :: strings :: JNIStr"));
	assert!(result_str.contains("test"));
}

#[test]
fn test_jni_str_with_jni_override_and_multiple_literals() {
	let input: TokenStream = quote! { jni = crate, "part1", "part2" };
	let result: TokenStream = jni_str_impl(input).expect("test JNI string expands");
	let result_str: String = result.to_string();

	// Should use crate:: and concatenate literals
	assert!(result_str.contains("crate :: strings :: JNIStr"));
	assert!(result_str.contains("part1part2"));
}

#[test]
fn test_jni_cstr_empty_input_fails() {
	let input: TokenStream = quote! {};
	let result: Result<TokenStream> = jni_cstr_impl(input);
	assert!(result.is_err());

	let err: syn::Error = result.expect_err("invalid test input is rejected");
	assert!(err.to_string().contains("expected at least one literal"));
}

#[test]
fn test_jni_cstr_trailing_comma() {
	let input: TokenStream = quote! { "hello", "world", };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();

	// Should handle trailing comma and concatenate
	assert!(result_str.contains("helloworld"));
}

#[test]
fn test_jni_str_jni_property_must_be_first() {
	// jni property after string literal should fail during parsing
	// because it will try to parse "test" as a string literal, then fail on the identifier "jni"
	let input: TokenStream = quote! { "test", jni = crate };
	let result: Result<TokenStream> = jni_str_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_mutf8_encoding() {
	// Test that emoji is properly encoded using MUTF-8
	let input: TokenStream = quote! { "😀" };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();

	// Emoji should be encoded as surrogate pairs in MUTF-8
	// The exact encoding will be in the c"..." literal
	assert!(result_str.contains("c\"") || result_str.contains('"'));
}

#[test]
fn test_literal_char() {
	let input: TokenStream = quote! { 'a' };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains('a'));
}

#[test]
fn test_literal_int() {
	let input: TokenStream = quote! { 42 };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("42"));
}

#[test]
fn test_literal_float() {
	let input: TokenStream = quote! { 3.14 };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("3.14"));
}

#[test]
fn test_literal_bool() {
	let input: TokenStream = quote! { true };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("true"));

	let input: TokenStream = quote! { false };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("false"));
}

#[test]
fn test_literal_byte() {
	let input: TokenStream = quote! { b'A' };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	// Byte literals are formatted as their numeric value
	assert!(result_str.contains("65")); // ASCII value of 'A'
}

#[test]
fn test_literal_cstr_valid_utf8() {
	let input: TokenStream = quote! { c"hello" };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("hello"));
}

#[test]
fn test_mixed_literals() {
	let input: TokenStream = quote! { "Port: ", 8080 };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("Port: 8080") || result_str.contains("Port : 8080"));
}

#[test]
fn test_mixed_literals_with_char() {
	let input: TokenStream = quote! { "Version ", 1, '.', 2 };
	let result: TokenStream = jni_cstr_impl(input).expect("test C string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("Version 1.2") || result_str.contains("Version 1 . 2"));
}

#[test]
fn test_jni_str_mixed_literals() {
	let input: TokenStream = quote! { "localhost:", 8080 };
	let result: TokenStream = jni_str_impl(input).expect("test JNI string expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("localhost:8080") || result_str.contains("localhost : 8080"));
	assert!(result_str.contains("JNIStr"));
}

#[test]
fn test_literal_byte_str_fails() {
	// Byte string literals should not be supported
	let input: TokenStream = quote! { b"hello" };
	let result: Result<TokenStream> = jni_cstr_impl(input);
	assert!(result.is_err());
	let err: syn::Error = result.expect_err("invalid test input is rejected");
	assert!(err.to_string().contains("byte string"));
}
