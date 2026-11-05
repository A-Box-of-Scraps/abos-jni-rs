use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use crate::internal;

pub fn jni_to_union(attr: &TokenStream, item: TokenStream) -> TokenStream {
	if !attr.is_empty() {
		return syn::Error::new(proc_macro2::Span::call_site(), "Expected no macro arguments")
			.into_compile_error()
			.into();
	}
	let input: DeriveInput = parse_macro_input!(item as DeriveInput);
	match internal::expand(&input) {
		Ok(tokens) => tokens.into(),
		Err(error) => error.into_compile_error().into(),
	}
}
