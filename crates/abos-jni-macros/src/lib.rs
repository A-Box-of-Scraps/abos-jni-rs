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
///
/// Generated methods and constructors check non-null reference arguments against
/// their resolved Java parameter classes. These classes are cached lazily per
/// binding, with the same process lifetime as the binding's cached class and
/// method IDs. Every call still checks assignability, including class-loader
/// identity; only the reflective parameter lookup is cached.
///
/// Custom primitive mappings such as `unsafe Handle => long` use `Into` conversions
/// when calling Java: arguments and field setters convert `Handle` into the JNI
/// primitive (`jlong`), while method returns and field getters convert it back.
/// Implementing `From<Handle> for jlong` and `From<jlong> for Handle` supplies both
/// directions. Only the conversions used by the declared members are required;
/// the wrapper does not need to be `Copy`.
///
/// Custom primitives must have the same size and alignment as their JNI primitive.
/// Built-in `bool` returns and getters normalize JNI booleans to `false` or `true`;
/// raw `jboolean` and custom boolean wrappers use the raw JNI byte instead.
#[proc_macro]
pub fn bind_java_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::bind_java_type(input)
}
