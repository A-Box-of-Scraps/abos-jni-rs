use super::{ConcreteType, SigType, TokenStream, TypeMappings, quote, sig_type_to_rust_type_core};

/// Whether a signature uses ergonomic Rust bool rather than raw jboolean.
pub fn is_rust_bool(ty: &SigType, mappings: &TypeMappings) -> bool {
	matches!(ty, SigType::Alias(alias) if matches!(mappings.map_alias(alias),
		Some(ConcreteType::Primitive { path, .. }) if path == "bool"))
}

/// Native boundaries always carry jboolean, even when the implementation uses bool.
pub fn native_type(ty: &SigType, lifetime: &TokenStream, mappings: &TypeMappings, jni: &syn::Path) -> TokenStream {
	if is_rust_bool(ty, mappings) {
		quote! { #jni::sys::jboolean }
	} else {
		sig_type_to_rust_type_core(ty, lifetime, mappings, jni)
	}
}

pub fn native_argument(ty: &SigType, name: &syn::Ident, mappings: &TypeMappings, jni: &syn::Path) -> TokenStream {
	if is_rust_bool(ty, mappings) {
		quote! { (#name != #jni::sys::JNI_FALSE) }
	} else {
		quote! { #name }
	}
}

pub fn native_return(ty: &SigType, body: TokenStream, mappings: &TypeMappings, jni: &syn::Path) -> TokenStream {
	if is_rust_bool(ty, mappings) {
		quote! { #jni::sys::jboolean::from({ #body }) }
	} else {
		body
	}
}
