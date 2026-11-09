//! Modified UTF-8 encoding and `CStr`/`JNIStr` literal generation for JNI macros.

use proc_macro2::Span;
use syn::LitCStr;

/// Encode UTF-8 as Java modified UTF-8: NUL becomes `0xC0 0x80`, and characters
/// above U+FFFF use CESU-8 surrogate pairs.
pub fn encode_mutf8(s: &str) -> Vec<u8> {
	simd_cesu8::mutf8::encode(s).into_owned()
}

/// Create a modified UTF-8 `LitCStr`, encoding embedded NULs without interior zero bytes.
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

/// Expand `jni_cstr!` literals into a concatenated, modified UTF-8 `CStr` literal.
pub fn jni_cstr_impl(input: TokenStream) -> Result<TokenStream> {
	let JniStrInput { string, .. } = syn::parse2(input)?;

	let cstr: LitCStr = lit_cstr_mutf8(&string);

	Ok(quote! {
		#cstr
	})
}

/// Expand `jni_str!` literals into a concatenated, modified UTF-8 `&'static JNIStr`.
pub fn jni_str_impl(input: TokenStream) -> Result<TokenStream> {
	let JniStrInput { jni_crate, string } = syn::parse2(input)?;

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
