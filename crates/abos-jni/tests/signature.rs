#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
use abos_jni::signature::{JavaType, Primitive};
use abos_jni_macros::jni_sig;

#[path = "signature/encoding.rs"]
mod encoding;
#[path = "signature/strings.rs"]
mod strings;
#[path = "signature/syntax.rs"]
mod syntax;
#[path = "signature/types.rs"]
mod types;
