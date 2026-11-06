use super::*;
use quote::quote;

// Test parsing error: single identifier without dot should fail
#[test]
fn test_single_ident_fails_as_java_class() {
	// This should be parsed as a Rust type reference, not a Java class
	let input = quote! {
		String
	};

	let result = jni_sig_impl(input);
	// Should fail because "String" without a dot is treated as a Rust type
	// and there's no type mapping for it
	assert!(result.is_err());
}

// Test that slash-separated package names fail with helpful error
// (Users should use dot-separated Java format, not JNI internal format)
#[test]
fn test_slash_separated_package_fails() {
	// This should fail because we expect Java dotted format (java.lang.String)
	// not JNI internal format (java/lang/String)
	let input = quote! {
		java/lang/String
	};

	let result = jni_sig_impl(input);
	// The parser will treat this as a division operation and fail to parse
	assert!(result.is_err());

	// Verify the error is about parsing, not about finding the type
	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	// The error should be a parse error, not a "type not found" error
	assert!(!err_msg.contains("No type mapping found"));
}

// Test that string literals don't support slash separators either
#[test]
fn test_string_literal_with_slashes_in_type_mapping() {
	// Even in string literals, we expect Java dotted format
	let input = quote! {
		MyCustomType,
		type_map = {
			MyCustomType => "java/lang/String",
		}
	};

	// This should fail during parsing of the type mapping
	// String literals with slashes should be rejected
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	// The error should mention that dots are required
	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	assert!(err_msg.contains("must contain at least one dot"));
}

// Test that the parser requires at least one dot to differentiate from Rust types
#[test]
fn test_no_dots_fails_for_java_class() {
	// Without dots, this should be treated as a Rust type reference
	let input = quote! {
		(arg: SomeType) -> void
	};

	let result = jni_sig_impl(input);
	// Should fail with "Unknown type" after attempting to resolve SomeType
	// via type mappings
	assert!(result.is_err());
	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	assert!(err_msg.contains("Unknown type"));
}

// Test that string literal in JavaClassName::parse requires dots
#[test]
fn test_string_literal_without_dots_fails() {
	let input = quote! { "String" };
	let result = syn::parse2::<JavaClassName>(input);

	assert!(result.is_err());
	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	assert!(err_msg.contains("must contain at least one dot"));
}

// Test that JavaClassName::parse rejects single identifiers without consuming them
#[test]
fn test_parse_java_class_name_rejects_single_ident() {
	// A single identifier without a dot should fail to parse as a Java class name
	let input = quote! { SomeType };
	let result = syn::parse2::<JavaClassName>(input);

	assert!(result.is_err());
	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	assert!(err_msg.contains("Expected Java class name"));
}

// ============================================================================
// Raw JNI signature tests
// ============================================================================

#[test]
fn test_raw_jni_field_primitives() {
	// Test all primitive types
	let primitives = [
		("Z", "Z", "Boolean"),
		("B", "B", "Byte"),
		("C", "C", "Char"),
		("S", "S", "Short"),
		("I", "I", "Int"),
		("J", "J", "Long"),
		("F", "F", "Float"),
		("D", "D", "Double"),
	];

	for (input_sig, expected_sig, expected_type) in primitives {
		let input = quote! { #input_sig };
		let result = jni_sig_impl(input).expect("test signature expands");
		let result_str = result.to_string();
		assert!(
			result_str.contains(&format!("\"{expected_sig}\"")),
			"Expected signature '{expected_sig}' for input '{input_sig}'"
		);
		assert!(
			result_str.contains(&format!("Primitive :: {expected_type}")),
			"Expected primitive type '{expected_type}' for input '{input_sig}'"
		);
		assert!(
			result_str.contains("FieldSignature"),
			"Expected FieldSignature output for input '{input_sig}'"
		);
	}
}

#[test]
fn test_raw_jni_errors_empty_string() {
	let input = quote! { "" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_invalid_primitive() {
	let input = quote! { "A" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_field() {
	let input = quote! { "V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_in_method_arguments() {
	// Void arrays are invalid in method arguments
	let input = quote! { "([V)V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	let input = quote! { "(I[V)V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	let input = quote! { "([VI)V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_as_return_type() {
	// Void arrays are invalid as return types
	let input = quote! { "()[V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	let input = quote! { "(I)[V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_field() {
	// Void arrays are invalid as field types
	let input = quote! { "[V" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_incomplete_method() {
	let input = quote! { "(I" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_method_no_return() {
	let input = quote! { "(I)" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_trailing_input() {
	let input = quote! { "II" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_invalid_in_method() {
	let input = quote! { "(Invalid)I" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_object_no_semicolon() {
	let input = quote! { "Ljava/lang/String" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_object_empty_name() {
	let input = quote! { "L;" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_leading_slash() {
	let input = quote! { "L/java/lang/String;" };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_string_with_dots_as_field() {
	// String literal with dots should be treated as Java class name for field
	let input = quote! { "java.lang.String" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"Ljava/lang/String;\""));
	assert!(result_str.contains("FieldSignature :: from_raw_parts"));
}

#[test]
fn test_raw_jni_inner_classes() {
	// Test inner classes with $ separator
	let input = quote! { "Ljava/lang/Outer$Inner;" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"Ljava/lang/Outer$Inner;\""));
}

#[test]
fn test_raw_jni_method_with_inner_class() {
	let input = quote! { "(Ljava/lang/Outer$Inner;)V" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"(Ljava/lang/Outer$Inner;)V\""));
}

#[test]
fn test_raw_jni_default_package_class() {
	// Default package class (no package)
	let input = quote! { "LNoPackage;" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"LNoPackage;\""));
}

#[test]
fn test_raw_jni_multidimensional_array() {
	let input = quote! { "[[I" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"[[I\""));
}

#[test]
fn test_raw_jni_method_returns_array() {
	let input = quote! { "()[[I" };
	let result = jni_sig_impl(input).expect("test signature expands");
	let result_str = result.to_string();
	assert!(result_str.contains("\"()[[I\""));
}

#[test]
fn test_non_raw_void_array_prefix_syntax() {
	// Test void array with prefix syntax [void]
	let input = quote! { [void] };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_suffix_syntax() {
	// Test void array with suffix syntax void[]
	let input = quote! { void[] };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_in_method_args() {
	// Test void array in method arguments
	let input = quote! { (arg: [void]) -> void };
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	let input = quote! { (arg: void[]) -> void };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_as_return_type() {
	// Test void array as return type
	let input = quote! { () -> [void] };
	let result = jni_sig_impl(input);
	assert!(result.is_err());

	let input = quote! { () -> void[] };
	let result = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_cstr_literal_invalid_utf8() {
	// CStr literals can contain byte escapes that create invalid UTF-8 sequences
	// For example, \xFF is not valid UTF-8
	// This tests that our code properly rejects such literals with a clear error message

	// Create a CStr literal with invalid UTF-8 using byte escapes
	// \xFF is not a valid UTF-8 byte sequence
	let input = quote! { c"(\xFF)V" };
	let result = jni_sig_impl(input);

	// Should fail with a UTF-8 validation error
	assert!(result.is_err(), "CStr literal with invalid UTF-8 should be rejected");

	let err = result.expect_err("invalid test input is rejected");
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("CStr literal must contain valid UTF-8"),
		"Error message should mention UTF-8 validation, got: {err_msg}"
	);

	// Also test that valid UTF-8 in CStr literals works correctly
	let input = quote! { c"(I)V" };
	let result = jni_sig_impl(input);
	assert!(result.is_ok(), "Valid UTF-8 CStr literal should parse successfully");

	let result_str = result.expect("test operation succeeds").to_string();
	assert!(result_str.contains("\"(I)V\""), "Should contain the signature string");
}
