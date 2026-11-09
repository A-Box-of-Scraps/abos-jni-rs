//! JNI export-name generation for the `jni_mangle` attribute.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use proc_macro2::TokenStream;
use quote::ToTokens;
use std::fmt::Write as _;
use syn::parse_quote;
use syn::spanned::Spanned;

/// Expand `jni_mangle` into a public system-ABI function with a JNI export name.
pub fn jni_mangle2(attr: TokenStream, item: TokenStream) -> TokenStream {
	let attr_span: proc_macro2::Span = attr.span();
	let item_span: proc_macro2::Span = item.span();

	let mut function: syn::ItemFn = match syn::parse2(item) {
		Ok(f) => f,
		Err(_e) => {
			return syn::Error::new(
				item_span,
				"The `jni_mangle` attribute can only be applied to `fn` items",
			)
			.to_compile_error();
		}
	};

	let args: syn::punctuated::Punctuated<syn::LitStr, syn::Token![,]> =
		match syn::parse::Parser::parse2(syn::punctuated::Punctuated::parse_terminated, attr) {
			Ok(args) => args,
			Err(_e) => {
				return syn::Error::new(
					attr_span,
					"The `jni_mangle` attribute must have string literal arguments",
				)
				.to_compile_error();
			}
		};

	if args.is_empty() || args.len() > 3 {
		return syn::Error::new(
			attr_span,
			"The `jni_mangle` attribute must have 1-3 string literal arguments",
		)
		.to_compile_error();
	}

	let namespace: String = args[0].value();

	if !valid_namespace(&namespace) {
		return syn::Error::new(
			attr_span,
			"Invalid package namespace supplied to `jni_mangle` attribute",
		)
		.to_compile_error();
	}

	let orig_fn_name: String = function.sig.ident.to_string();

	let (method_name, signature) = match args.len() {
		1 => (snake_case_to_lower_camel_case(&orig_fn_name), None),
		2 => {
			let second_arg: String = args[1].value();
			if second_arg.contains('(') {
				(snake_case_to_lower_camel_case(&orig_fn_name), Some(second_arg))
			} else {
				(second_arg, None)
			}
		}
		3 => (args[1].value(), Some(args[2].value())),
		_ => unreachable!(),
	};

	let mangled_jni_name: String = create_jni_fn_name(&namespace, &method_name, signature.as_deref());

	// Set the export symbol without changing the function's Rust name, so Rust callers can still use it.
	if cfg!(has_unsafe_attr) {
		function
			.attrs
			.extend([parse_quote!(#[unsafe(export_name = #mangled_jni_name)])]);
	} else {
		function
			.attrs
			.extend([parse_quote!(#[export_name = #mangled_jni_name])]);
	}

	function.attrs.extend([parse_quote!(#[allow(non_snake_case)])]);

	if let Err(error) = set_system_abi(&mut function) {
		return error.to_compile_error();
	}

	if !matches!(function.vis, syn::Visibility::Public(_)) {
		return syn::Error::new(
			function.vis.span(),
			"`jni_mangle` attributed functions must have public visibility (`pub`)",
		)
		.to_compile_error();
	}

	function.into_token_stream()
}

fn set_system_abi(function: &mut syn::ItemFn) -> syn::Result<()> {
	if let Some(abi) = &function.sig.abi
		&& let Some(name) = &abi.name
	{
		if name.value() != "system" {
			return Err(syn::Error::new(
				name.span(),
				format!(
					"`jni_mangle` attributed functions must use `extern \"system\"` ABI, found `extern \"{}\"`",
					name.value()
				),
			));
		}
		return Ok(());
	}
	let extern_token: syn::token::Extern = function
		.sig
		.abi
		.as_ref()
		.map_or_else(syn::token::Extern::default, |abi| abi.extern_token);
	function.sig.abi = Some(syn::Abi {
		extern_token,
		name: Some(syn::LitStr::new("system", function.sig.ident.span())),
	});
	Ok(())
}

/// Reject obviously invalid dot-separated package names without enforcing Java-only
/// identifier rules, since other JVM languages may use different naming conventions.
fn valid_namespace(namespace: &str) -> bool {
	fn is_valid_ident(ident: &str) -> bool {
		const FORBIDDEN_START_CHARS: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

		if ident.is_empty() {
			return false;
		}

		for c in FORBIDDEN_START_CHARS {
			if ident.starts_with(*c) {
				return false;
			}
		}

		true
	}

	const FORBIDDEN_CHARS: &[char] = &[
		' ', ',', ':', ';', '|', '\\', '/', '!', '@', '#', '%', '^', '&', '*', '(', ')', '{', '}', '[', ']', '-', '`',
		'~', '\t', '\n', '\r',
	];

	for c in FORBIDDEN_CHARS {
		if namespace.contains(*c) {
			return false;
		}
	}

	if namespace.starts_with('.') || namespace.ends_with('.') {
		return false;
	}

	if namespace.contains("..") {
		return false;
	}

	for ident in namespace.split('.') {
		if !is_valid_ident(ident) {
			return false;
		}
	}

	true
}

/// Convert `snake_case` to lowerCamelCase, preserving names that contain uppercase letters.
///
/// Removes one leading underscore and preserves trailing underscores; all-underscore names
/// are unchanged. After each internal underscore, capitalizes the first non-ASCII-digit
/// character using Unicode casing (e.g., `array_2d_foo` becomes `array2DFoo`).
pub fn snake_case_to_lower_camel_case(s: &str) -> String {
	// Preserve intentional mixed or uppercase naming.
	if s.chars().any(char::is_uppercase) {
		return s.to_string();
	}

	let leading_underscores: usize = s.chars().take_while(|&c| c == '_').count();

	let trailing_underscores: usize = s.chars().rev().take_while(|&c| c == '_').count();

	if leading_underscores + trailing_underscores >= s.len() {
		return s.to_string();
	}

	let middle: &str = &s[leading_underscores..s.len() - trailing_underscores];

	let mut result = String::new();
	let mut capitalize_next = false;

	for c in middle.chars() {
		if c == '_' {
			capitalize_next = true;
		} else if capitalize_next {
			if c.is_ascii_digit() {
				result.push(c);
			} else {
				result.extend(c.to_uppercase());
				capitalize_next = false;
			}
		} else {
			result.push(c);
		}
	}

	let adjusted_leading_underscores: usize = if leading_underscores > 0 {
		leading_underscores - 1
	} else {
		0
	};

	let mut final_result: String = String::with_capacity(s.len());
	for _ in 0..adjusted_leading_underscores {
		final_result.push('_');
	}
	final_result.push_str(&result);
	for _ in 0..trailing_underscores {
		final_result.push('_');
	}

	final_result
}

/// Creates a JNI-compatible function name from the given namespace, function name, and optional signature.
/// Preserves the method name's casing.
///
/// Any underscores in the original namespace or function name need to be replaced by "_1", and
/// then dot separators need to be turned into underscores.
///
/// For signatures (if provided), only the argument types (between parentheses) are encoded:
/// - '_' -> "_1"
/// - ';' -> "_2"
/// - '[' -> "_3"
/// - '/' -> "_"
/// - Other characters, including '$', -> "_0xxxx" with the lowercase hex Unicode codepoint
///
/// The return type is ignored, and parentheses are not included in the mangled name.
pub fn create_jni_fn_name(namespace: &str, fn_name: &str, signature: Option<&str>) -> String {
	fn mangle_identifier(s: &str) -> String {
		let mut result = String::new();
		for c in s.chars() {
			match c {
				'_' => result.push_str("_1"),
				'.' => result.push('_'),
				_ if c.is_ascii_alphanumeric() => result.push(c),
				_ => write!(result, "_0{:04x}", u32::from(c)).expect("writing to String cannot fail"),
			}
		}
		result
	}

	fn mangle_signature_args(s: &str) -> String {
		let mut result = String::new();
		for c in s.chars() {
			match c {
				'_' => result.push_str("_1"),
				';' => result.push_str("_2"),
				'[' => result.push_str("_3"),
				'/' => result.push('_'),
				_ if c.is_ascii_alphanumeric() => result.push(c),
				_ => {
					write!(result, "_0{:04x}", u32::from(c)).expect("writing to String cannot fail");
				}
			}
		}
		result
	}

	let namespace_underscored: String = mangle_identifier(namespace);
	let fn_name_underscored: String = mangle_identifier(fn_name);

	let mut result: String = format!("Java_{namespace_underscored}_{fn_name_underscored}");

	if let Some(sig) = signature
		&& let Some(start) = sig.find('(')
		&& let Some(end) = sig.find(')')
	{
		let args: &str = &sig[start + 1..end];
		// Always add __ when signature is provided (indicates overloaded method)
		result.push_str("__");
		if !args.is_empty() {
			result.push_str(&mangle_signature_args(args));
		}
	}

	result
}

#[cfg(test)]
#[path = "../../../tests/unit/internal/mangle.rs"]
mod tests;
