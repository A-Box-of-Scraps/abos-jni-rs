#![doc = include_str!("../CHANGELOG.md")]
#![doc = "\n\n[Unreleased]: #unreleased\n"]
#![allow(
	missing_docs,
	reason = "Internal macros implementing versioned JNI and JVMTI table representations."
)]

mod api;
mod internal;

use proc_macro::TokenStream;

// Procedural macro entry points must remain at the crate root.
#[proc_macro_attribute]
pub fn jni_to_union(attr: TokenStream, item: TokenStream) -> TokenStream {
	api::jni_to_union(&attr, item)
}

/// Retains a raw JVMTI table and generates historical views from `jvmti_added` attributes.
#[proc_macro_attribute]
pub fn jvmti_versions(attr: TokenStream, item: TokenStream) -> TokenStream {
	api::jvmti_versions(&attr, item)
}
