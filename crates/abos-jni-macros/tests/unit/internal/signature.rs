use super::*;
use quote::quote;

#[test]
fn test_single_ident_fails_as_java_class() {
	let input: proc_macro2::TokenStream = quote! {
		String
	};

	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	// Should fail because "String" without a dot is treated as a Rust type
	// and there's no type mapping for it
	assert!(result.is_err());
}

#[test]
fn test_slash_separated_package_fails() {
	let input: proc_macro2::TokenStream = quote! {
		java/lang/String
	};

	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(!err_msg.contains("No type mapping found"));
}

#[test]
fn test_string_literal_with_slashes_in_type_mapping() {
	let input: proc_macro2::TokenStream = quote! {
		MyCustomType,
		type_map = {
			MyCustomType => "java/lang/String",
		}
	};

	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(err_msg.contains("must contain at least one dot"));
}

#[test]
fn test_no_dots_fails_for_java_class() {
	let input: proc_macro2::TokenStream = quote! {
		(arg: SomeType) -> void
	};

	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(err_msg.contains("Unknown type"));
}

#[test]
fn test_string_literal_without_dots_fails() {
	let input: proc_macro2::TokenStream = quote! { "String" };
	let result: Result<JavaClassName> = syn::parse2::<JavaClassName>(input);

	assert!(result.is_err());
	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(err_msg.contains("must contain at least one dot"));
}

#[test]
fn test_parse_java_class_name_rejects_single_ident() {
	let input: proc_macro2::TokenStream = quote! { SomeType };
	let result: Result<JavaClassName> = syn::parse2::<JavaClassName>(input);

	assert!(result.is_err());
	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(err_msg.contains("Expected Java class name"));
}

#[test]
fn test_raw_jni_field_primitives() {
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
		let input: proc_macro2::TokenStream = quote! { #input_sig };
		let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
		let result_str: String = result.to_string();
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
	let input: proc_macro2::TokenStream = quote! { "" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_invalid_primitive() {
	let input: proc_macro2::TokenStream = quote! { "A" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_field() {
	let input: proc_macro2::TokenStream = quote! { "V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_in_method_arguments() {
	let input: proc_macro2::TokenStream = quote! { "([V)V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let input: proc_macro2::TokenStream = quote! { "(I[V)V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let input: proc_macro2::TokenStream = quote! { "([VI)V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_as_return_type() {
	let input: proc_macro2::TokenStream = quote! { "()[V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let input: proc_macro2::TokenStream = quote! { "(I)[V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_void_array_field() {
	let input: proc_macro2::TokenStream = quote! { "[V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_incomplete_method() {
	let input: proc_macro2::TokenStream = quote! { "(I" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_method_no_return() {
	let input: proc_macro2::TokenStream = quote! { "(I)" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_trailing_input() {
	let input: proc_macro2::TokenStream = quote! { "II" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_invalid_in_method() {
	let input: proc_macro2::TokenStream = quote! { "(Invalid)I" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_object_no_semicolon() {
	let input: proc_macro2::TokenStream = quote! { "Ljava/lang/String" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_object_empty_name() {
	let input: proc_macro2::TokenStream = quote! { "L;" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_errors_leading_slash() {
	let input: proc_macro2::TokenStream = quote! { "L/java/lang/String;" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_raw_jni_string_with_dots_as_field() {
	let input: proc_macro2::TokenStream = quote! { "java.lang.String" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"Ljava/lang/String;\""));
	assert!(result_str.contains("FieldSignature :: from_raw_parts"));
}

#[test]
fn test_raw_jni_inner_classes() {
	let input: proc_macro2::TokenStream = quote! { "Ljava/lang/Outer$Inner;" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"Ljava/lang/Outer$Inner;\""));
}

#[test]
fn test_raw_jni_method_with_inner_class() {
	let input: proc_macro2::TokenStream = quote! { "(Ljava/lang/Outer$Inner;)V" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"(Ljava/lang/Outer$Inner;)V\""));
}

#[test]
fn test_raw_jni_default_package_class() {
	let input: proc_macro2::TokenStream = quote! { "LNoPackage;" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"LNoPackage;\""));
}

#[test]
fn test_raw_jni_multidimensional_array() {
	let input: proc_macro2::TokenStream = quote! { "[[I" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"[[I\""));
}

#[test]
fn test_raw_jni_method_returns_array() {
	let input: proc_macro2::TokenStream = quote! { "()[[I" };
	let result: proc_macro2::TokenStream = jni_sig_impl(input).expect("test signature expands");
	let result_str: String = result.to_string();
	assert!(result_str.contains("\"()[[I\""));
}

#[test]
fn test_non_raw_void_array_prefix_syntax() {
	let input: proc_macro2::TokenStream = quote! { [void] };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_suffix_syntax() {
	let input: proc_macro2::TokenStream = quote! { void[] };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_in_method_args() {
	let input: proc_macro2::TokenStream = quote! { (arg: [void]) -> void };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let input: proc_macro2::TokenStream = quote! { (arg: void[]) -> void };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_non_raw_void_array_as_return_type() {
	let input: proc_macro2::TokenStream = quote! { () -> [void] };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());

	let input: proc_macro2::TokenStream = quote! { () -> void[] };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_err());
}

#[test]
fn test_cstr_literal_invalid_utf8() {
	// C string literals allow byte escapes that are not valid UTF-8.

	let input: proc_macro2::TokenStream = quote! { c"(\xFF)V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);

	assert!(result.is_err(), "CStr literal with invalid UTF-8 should be rejected");

	let err: syn::Error = result.expect_err("invalid test input is rejected");
	let err_msg: String = err.to_string();
	assert!(
		err_msg.contains("CStr literal must contain valid UTF-8"),
		"Error message should mention UTF-8 validation, got: {err_msg}"
	);

	let input: proc_macro2::TokenStream = quote! { c"(I)V" };
	let result: Result<proc_macro2::TokenStream> = jni_sig_impl(input);
	assert!(result.is_ok(), "Valid UTF-8 CStr literal should parse successfully");

	let result_str: String = result.expect("test operation succeeds").to_string();
	assert!(result_str.contains("\"(I)V\""), "Should contain the signature string");
}
