/// `jni-sys` re-exports
pub use abos_jni_sys as sys;

mod version;
pub use self::version::*;

/// Error types and utilities.
pub mod errors;

/// Parser for java type signatures.
pub mod signature;

/// Descriptors for classes and method IDs.
pub mod descriptors;

// Wrappers around jni value types that add lifetimes and other functionality.
mod jvalue;
pub use jvalue::{JValue, JValueOwned};

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use `abos_jni::string::{char_from_*, char_to_*}` instead of `abos_jni::{char_from_*, char_to_*}`"
)]
pub use jvalue::{char_from_java, char_from_java_int, char_to_java, char_to_java_int};

/// JNI identifiers for methods and fields
pub mod ids;

/// Base support for reference types and auto-release wrappers like [`refs::Auto`] and [`refs::Global`].
pub mod refs;

/// Reference type implementations and API bindings for various `java.lang` Objects
pub mod objects;

/// Reference type implementations and API bindings for various `java.lang` Exceptions
pub mod exceptions;

/// Helpers for accessing array elements
pub mod elements;

/// Handling of strings in Java's [modified UTF-8] encoding, including
/// conversion to and from Rust strings (which use standard UTF-8).
///
/// [modified UTF-8]: https://en.wikipedia.org/wiki/UTF-8#Modified_UTF-8
pub mod strings;

mod env;
pub use env::*;

#[deprecated(
	since = "0.22.0",
	note = r#"Since 0.22, `JNIEnv` has been split into `Env` and `EnvUnowned`

- `EnvUnowned` is FFI safe and can be accepted as an argument for native methods
- `Env` is what provides the full JNI environment API and is not FFI safe.

To remain safe by default, `abos_jni::JNIEnv` is now an alias for `EnvUnowned` (which is FFI safe).

Use `EnvUnowned` to capture a raw `abos_jni_sys::JNIEnv` pointer in native methods, like:
`pub extern "system" fn Java_HelloWorld_hello<'frame>(unowned_env: EnvUnowned<'frame>, ...)`
Then use `unowned_env.with_env()` to upgrade it to an `Env` reference.

Most of the time you should temporarily acquire a `Env` reference using:
- `JavaVM::attach_current_thread` (preferred) or `JavaVM::attach_current_thread_for_scope`
- `JavaVM::with_env` (if certain that the thread is already attached)
- `UnownedEnv::with_env()` in native methods

When migrating to 0.22, if you have a mixture of FFI/non-FFI usage of JNIEnv then err on the side
of renaming `JNIEnv` to `EnvUnowned` because that's safe for FFI and for non-FFI usage there will be
clear compiler errors if trying to access the real `Env` API through the `EnvUnowned` type.
"#
)]
/// An FFI safe alias for `EnvUnowned` for (safer) compatibility with
/// existing code.
///
/// Since 0.22 ([#570](https://github.com/jni-rs/jni-rs/pull/570)) the
/// `JNIEnv` type was renamed to [Env], which is no longer an FFI safe
/// pointer wrapper.
///
/// FFI usage of `JNIEnv`, within native method arguments, should be
/// migrated to [EnvUnowned], followed by [`EnvUnowned::with_env`].
///
/// To help make this clear and sign post how to safely migrate to the [Env]
/// and [EnvUnowned] types, we export this deprecated alias with a warning.
pub type JNIEnv<'frame> = EnvUnowned<'frame>;

/// Java VM interface.
pub mod vm;

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use `abos_jni::vm::JavaVM` instead of `abos_jni::JavaVM`."
)]
pub use self::vm::*;

#[doc(inline)]
pub use abos_jni_macros::jni_cstr;
pub use abos_jni_macros::jni_str;

pub use abos_jni_macros::jni_mangle;

pub use abos_jni_macros::jni_sig;
#[doc(inline)]
pub use abos_jni_macros::jni_sig_cstr;
#[doc(inline)]
pub use abos_jni_macros::jni_sig_jstr;
#[doc(inline)]
pub use abos_jni_macros::jni_sig_str;

pub use abos_jni_macros::bind_java_type;

pub use abos_jni_macros::native_method;

// For internal jni call macros to ensure their results are checked
#[doc(hidden)]
#[must_use]
pub fn __must_use<T>(v: T) -> T {
	v
}

// Provides a way to validate all our object bindings in a unit test, considering
// that the `J<Foo>API::get` functions are only exported with `pub(crate)` visibility.
//
// If any typos in method names or incorrect method or field signatures will result
// in a panic here when the binding initialization fails.
#[doc(hidden)]
pub fn __test_bindings_init(env: &crate::Env, loader: &crate::refs::LoaderContext) {
	objects::JByteBufferAPI::get(env, loader).expect("Failed to initialize JByteBufferAPI bindings");
	objects::JClassLoaderAPI::get(env, loader).expect("Failed to initialize JClassLoaderAPI bindings");
	objects::JClassAPI::get(env, loader).expect("Failed to initialize JClassAPI bindings");
	objects::JCharSequenceAPI::get(env, loader).expect("Failed to initialize JCharSequenceAPI bindings");
	objects::JCollectionAPI::get(env, loader).expect("Failed to initialize JCollectionAPI bindings");
	objects::JIteratorAPI::get(env, loader).expect("Failed to initialize JIteratorAPI bindings");
	objects::JListAPI::get(env, loader).expect("Failed to initialize JListAPI bindings");
	objects::JMapAPI::get(env, loader).expect("Failed to initialize JMapAPI bindings");
	objects::JMapEntryAPI::get(env, loader).expect("Failed to initialize JMapEntryAPI bindings");
	objects::JObjectArrayAPI::<objects::JString>::get(env, loader)
		.expect("Failed to initialize JObjectArrayAPI<JString> bindings");
	objects::JObjectAPI::get(env).expect("Failed to initialize JObjectAPI bindings");
	objects::JPrimitiveArrayAPI_jboolean::get(env, loader)
		.expect("Failed to initialize JPrimitiveArrayAPI_jboolean bindings");
	objects::JSetAPI::get(env, loader).expect("Failed to initialize JSetAPI bindings");
	objects::JStackTraceElementAPI::get(env, loader).expect("Failed to initialize JStackTraceElementAPI bindings");
	objects::JStringAPI::get(env, loader).expect("Failed to initialize JStringAPI bindings");
	objects::JThreadAPI::get(env, loader).expect("Failed to initialize JThreadAPI bindings");
	objects::JThrowableAPI::get(env, loader).expect("Failed to initialize JThrowableAPI bindings");

	exceptions::JArrayIndexOutOfBoundsExceptionAPI::get(env, loader)
		.expect("Failed to initialize JArrayIndexOutOfBoundsException bindings");
	exceptions::JArrayStoreExceptionAPI::get(env, loader).expect("Failed to initialize JArrayStoreException bindings");
	exceptions::JClassCircularityErrorAPI::get(env, loader)
		.expect("Failed to initialize JClassCircularityError bindings");
	exceptions::JClassFormatErrorAPI::get(env, loader).expect("Failed to initialize JClassFormatError bindings");
	exceptions::JExceptionInInitializerErrorAPI::get(env, loader)
		.expect("Failed to initialize JExceptionInInitializerError bindings");
	exceptions::JIllegalArgumentExceptionAPI::get(env, loader)
		.expect("Failed to initialize JIllegalArgumentException bindings");
	exceptions::JIllegalMonitorStateExceptionAPI::get(env, loader)
		.expect("Failed to initialize JIllegalMonitorStateException bindings");
	exceptions::JInstantiationExceptionAPI::get(env, loader)
		.expect("Failed to initialize JInstantiationException bindings");
	exceptions::JNoClassDefFoundErrorAPI::get(env, loader)
		.expect("Failed to initialize JNoClassDefFoundError bindings");
	exceptions::JNoSuchFieldErrorAPI::get(env, loader).expect("Failed to initialize JNoSuchFieldError bindings");
	exceptions::JNoSuchMethodErrorAPI::get(env, loader).expect("Failed to initialize JNoSuchMethodError bindings");
	exceptions::JOutOfMemoryErrorAPI::get(env, loader).expect("Failed to initialize JOutOfMemoryError bindings");
	exceptions::JSecurityExceptionAPI::get(env, loader).expect("Failed to initialize JSecurityException bindings");
	exceptions::JStringIndexOutOfBoundsExceptionAPI::get(env, loader)
		.expect("Failed to initialize JStringIndexOutOfBoundsException bindings");
}
