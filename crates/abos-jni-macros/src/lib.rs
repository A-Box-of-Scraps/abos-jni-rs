#![doc = include_str!("../README.md")]

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
///
/// A generated descriptor does not retain the native library containing its wrapper or
/// implementation. When registering it, satisfy the code-lifetime requirements of
/// `Env::register_native_methods`; wrapper generation does not manage library unloading.
#[proc_macro]
pub fn native_method(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	api::native_method(input)
}

/// Generate Rust bindings for a Java type.
///
/// # Native library lifetime
///
/// `native_methods` are registered automatically during binding initialization, before Java
/// class initialization. The application must keep the generated wrappers and their native
/// implementations executable while calls can occur, including until all in-flight calls return.
/// The macro does not retain a native library handle or its owning class loader. Its cached
/// global reference to the target class is not sufficient if a different loader owns the library.
///
/// Use process-lifetime native code or independently prevent the library from unloading.
/// Automatic registration does not support coordinated unloading: it neither unregisters on
/// drop nor waits for active calls, and later initialization errors do not roll back earlier
/// registrations. Required and runtime-gated natives have the same lifetime obligation.
/// Manual unregistration alone is not a safe unloading protocol; new calls and concurrent
/// registration must also be prevented and all in-flight calls must have returned.
///
/// # Runtime member requirements
///
/// Member property blocks accept `requires = available` to tolerate an absent constructor,
/// method, field, or native declaration without disabling unrelated members. They also
/// accept `requires = predicate`, where the function or closure takes `&mut Env` and returns
/// `errors::Result<bool>`: false skips lookup/registration, while true requires it to succeed.
/// Unavailable Rust calls return `Error::MemberUnavailable` before using a JNI ID.
/// Other lookup, registration, and predicate errors still fail API initialization.
///
/// Availability is cached with the API. Predicates must describe stable capabilities and
/// tolerate retries and concurrent initialization. They must not re-enter this binding;
/// native predicates must not initialize its Java class before registration completes.
/// Gated natives suppress default exports and reject explicit exports, which would bypass
/// registration requirements. Their Rust implementations are still required at compile time.
/// Android API levels and Java library versions must not be inferred from JNI interface versions.
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
