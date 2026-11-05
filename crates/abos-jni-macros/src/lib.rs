//! Procedural macros for abos-jni.

mod api;
mod internal;

/// Encode string literals as a modified UTF-8 JNI string.
#[proc_macro]
pub fn jni_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_str(input)
}

/// Encode string literals as a modified UTF-8 C string.
#[proc_macro]
pub fn jni_cstr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_cstr(input)
}

/// Parse a JNI signature at compile time.
#[proc_macro]
pub fn jni_sig(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_sig(input)
}

/// Parse a JNI signature into a UTF-8 string literal.
#[proc_macro]
pub fn jni_sig_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_sig_str(input)
}

/// Parse a JNI signature into a C string literal.
#[proc_macro]
pub fn jni_sig_cstr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_sig_cstr(input)
}

/// Parse a JNI signature into a JNI string literal.
#[proc_macro]
pub fn jni_sig_jstr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_sig_jstr(input)
}

/// Export a native function using JNI name mangling.
#[proc_macro_attribute]
pub fn jni_mangle(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::jni_mangle(attr, item)
}

/// Generate a native method wrapper.
#[proc_macro]
pub fn native_method(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::native_method(input)
}

/// Generate Rust bindings for a Java type.
#[proc_macro]
pub fn bind_java_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::bind_java_type(input)
}
