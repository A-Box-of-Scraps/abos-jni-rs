//! String utilities for JNI signature and class name handling
//!
//! This module provides utilities for encoding strings to Java's Modified UTF-8 (MUTF-8)
//! format and creating `CStr` and `JNIStr` literals for use in procedural macros.

use proc_macro2::Span;
use syn::LitCStr;

/// Encode a UTF-8 string as MUTF-8 (Java's modified UTF-8)
///
/// Java uses a modified version of UTF-8 that:
/// - Encodes the null character (U+0000) as `0xC0 0x80` instead of `0x00`
/// - Encodes Unicode characters above U+FFFF using CESU-8 (surrogate pairs)
///
/// # Examples
///
/// ```ignore
/// let mutf8_bytes = encode_mutf8("Hello");
/// // Basic ASCII is unchanged
///
/// let mutf8_emoji = encode_mutf8("😀");
/// // Emoji encoded as surrogate pairs in MUTF-8
/// ```
pub fn encode_mutf8(s: &str) -> Vec<u8> {
	simd_cesu8::mutf8::encode(s).into_owned()
}

/// Create a `LitCStr` from a string with MUTF-8 encoding
///
/// This function takes a UTF-8 string, encodes it to MUTF-8, and creates
/// a `syn::LitCStr` for use in procedural macros.
///
/// Modified UTF-8 encodes embedded NUL characters as `\xC0\x80`, without zero bytes.
///
/// # Examples
///
/// ```ignore
/// let cstr = lit_cstr_mutf8("java.lang.String");
/// // Creates c"java.lang.String" with MUTF-8 encoding
/// ```
pub fn lit_cstr_mutf8(s: &str) -> LitCStr {
	let mutf8: Vec<u8> = encode_mutf8(s);
	let c: std::ffi::CString = std::ffi::CString::new(mutf8).expect("Modified UTF-8 contains no interior NUL bytes");
	LitCStr::new(&c, Span::call_site())
}

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Lit, Result, Token, parse::Parse};

/// Convert a literal to a string value for concatenation
fn literal_to_string(lit: &Lit) -> Result<String> {
	match lit {
		Lit::Str(s) => Ok(s.value()),
		Lit::Char(c) => Ok(c.value().to_string()),
		Lit::Int(i) => Ok(i.base10_digits().to_string()),
		Lit::Float(f) => Ok(f.base10_digits().to_string()),
		Lit::Bool(b) => Ok(b.value.to_string()),
		Lit::Byte(b) => {
			// Bytes are formatted as their numeric value
			Ok(b.value().to_string())
		}
		Lit::CStr(c) => {
			// CStr must be valid UTF-8 for our purposes
			let cstr: std::ffi::CString = c.value();
			cstr.to_str()
				.map(std::string::ToString::to_string)
				.map_err(|e| syn::Error::new_spanned(c, format!("CStr literal must contain valid UTF-8: {e}")))
		}
		Lit::ByteStr(_) => Err(syn::Error::new_spanned(
			lit,
			"byte string literals are not supported in jni_str!/jni_cstr! macros",
		)),
		Lit::Verbatim(_) => Err(syn::Error::new_spanned(
			lit,
			"verbatim literals are not supported in jni_str!/jni_cstr! macros",
		)),
		// Lit is marked as non_exhaustive, so we need a wildcard pattern
		_ => Err(syn::Error::new_spanned(
			lit,
			"unsupported literal type in jni_str!/jni_cstr! macros",
		)),
	}
}

/// Input for `jni_cstr`! and `jni_str`! macros
/// Supports:
/// - Multiple comma-separated literals (like concat!)
/// - Optional `jni = <path>` as the first argument
struct JniStrInput {
	jni_crate: syn::Path,
	string: String,
}

impl Parse for JniStrInput {
	fn parse(input: syn::parse::ParseStream) -> Result<Self> {
		let jni_crate: syn::Path = crate::internal::utils::parse_jni_crate_override(input)?;

		// Parse comma-separated literals (like concat!)
		let mut concatenated = String::new();

		if input.is_empty() {
			return Err(syn::Error::new(input.span(), "expected at least one literal"));
		}

		loop {
			let lit: Lit = input.parse::<Lit>()?;
			let value: String = literal_to_string(&lit)?;
			concatenated.push_str(&value);

			if input.is_empty() {
				break;
			}

			input.parse::<Token![,]>()?;

			if input.is_empty() {
				break;
			}
		}

		Ok(Self {
			jni_crate,
			string: concatenated,
		})
	}
}

/// Implementation for `jni_cstr`! macro
/// Takes UTF-8 string literals and returns a MUTF-8 encoded `CStr` literal
/// Supports:
/// - Multiple comma-separated string literals (like concat!)
/// - Optional `jni = <path>` as the first argument
pub fn jni_cstr_impl(input: TokenStream) -> Result<TokenStream> {
	let JniStrInput { string, .. } = syn::parse2(input)?;

	// Create a C string literal with MUTF-8 encoding
	let cstr: LitCStr = lit_cstr_mutf8(&string);

	Ok(quote! {
		#cstr
	})
}

/// Implementation for `jni_str`! macro
/// Takes UTF-8 string literals and returns a MUTF-8 encoded &'static `JNIStr`
/// Supports:
/// - Multiple comma-separated string literals (like concat!)
/// - Optional `jni = <path>` as the first argument
pub fn jni_str_impl(input: TokenStream) -> Result<TokenStream> {
	let JniStrInput { jni_crate, string } = syn::parse2(input)?;

	// Create a C string literal with MUTF-8 encoding
	let cstr: LitCStr = lit_cstr_mutf8(&string);

	Ok(quote! {
		{
			// Safety: The string was encoded to MUTF-8 at compile-time
			unsafe {
				#jni_crate::strings::JNIStr::from_cstr_unchecked(#cstr)
			}
		}
	})
}

#[cfg(test)]
#[path = "../../tests/unit/internal/str.rs"]
mod tests;
