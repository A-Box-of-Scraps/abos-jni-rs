#![allow(
	missing_docs,
	reason = "Internal macro implementing the raw JNI table representation."
)]

mod api;
mod internal;

use proc_macro::TokenStream;

// Procedural macro entry points must remain at the crate root.
#[proc_macro_attribute]
pub fn jni_to_union(attr: TokenStream, item: TokenStream) -> TokenStream {
	api::jni_to_union(&attr, item)
}
