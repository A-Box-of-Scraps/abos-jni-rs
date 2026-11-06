use super::{NativeMethodExport, NativeMethodStructInput, native_method_impl};
use quote::quote;

#[test]
fn explicit_function_overrides_shorthand_in_either_order() {
	for tokens in [
		quote! { fn = explicit_impl, fn Example::java_method() -> void },
		quote! { fn Example::java_method() -> void, fn = explicit_impl },
	] {
		let parsed: NativeMethodStructInput = syn::parse2(tokens).expect("valid method declaration");
		assert!(parsed.fn_path.is_ident("explicit_impl"));
		assert_eq!(parsed.java_method_name, "javaMethod");
	}
}

#[test]
fn qualified_shorthand_preserves_owner_path() {
	let parsed: NativeMethodStructInput = syn::parse2(quote! {
		jni = ::renamed_jni,
		fn ::module::Example::java_method() -> void,
	})
	.expect("valid qualified method");
	let owner = parsed.rust_type.expect("qualified method has an owner");
	assert!(owner.leading_colon.is_some());
	assert_eq!(owner.segments.len(), 2);
	assert!(!owner.segments.trailing_punct());
	assert_eq!(parsed.fn_path.segments.len(), 3);
	assert!(parsed.jni_crate.segments[0].ident == "renamed_jni");
}

#[test]
fn named_export_does_not_require_java_type() {
	let tokens = native_method_impl(quote! {
		name = "method",
		sig = () -> void,
		fn = implementation,
		export = "custom_symbol",
	})
	.expect("explicit export name needs no Java class for mangling");
	let _: syn::Expr = syn::parse2(tokens).expect("expansion is a valid Rust expression");
}

#[test]
fn automatic_export_requires_java_type() {
	for tokens in [
		quote! { name = "method", sig = () -> void, fn = implementation, export = true },
		quote! { extern fn Example::method() -> void },
	] {
		let error = syn::parse2::<NativeMethodStructInput>(tokens)
			.err()
			.expect("missing Java type is rejected");
		assert!(error.to_string().contains("java_type"));
	}
}

#[test]
fn named_export_takes_precedence_over_extern_qualifier() {
	let parsed: NativeMethodStructInput = syn::parse2(quote! {
		export = "custom_symbol",
		extern fn Example::method() -> void,
	})
	.expect("named export can use extern shorthand without a Java type");
	assert!(matches!(parsed.export, NativeMethodExport::WithName(ref name) if name == "custom_symbol"));
}

#[test]
fn raw_methods_reject_wrapper_options() {
	for (tokens, property) in [
		(
			quote! { raw fn Example::method() -> void, error_policy = Policy },
			"error_policy",
		),
		(
			quote! { raw fn Example::method() -> void, catch_unwind = false },
			"catch_unwind",
		),
	] {
		let error = syn::parse2::<NativeMethodStructInput>(tokens)
			.err()
			.expect("raw method cannot use wrapper options");
		assert!(error.to_string().contains(property));
	}
}

#[test]
fn missing_function_reports_a_parse_error() {
	let error = syn::parse2::<NativeMethodStructInput>(quote! {
		name = "method", sig = () -> void,
	})
	.err()
	.expect("a method must identify an implementation");
	assert!(error.to_string().contains("Missing 'fn'"));
}
