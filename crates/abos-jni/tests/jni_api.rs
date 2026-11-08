#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
use std::convert::TryFrom;

use assert_matches::assert_matches;

use abos_jni::{
	Env,
	descriptors::Desc,
	errors::{CharToJavaError, Error, JniError},
	jni_sig, jni_str,
	objects::{
		AutoElements, IntoAuto as _, JByteBuffer, JList, JObject, JObjectArray, JStackTraceElement, JString,
		JThrowable, JValue, ReleaseMode, Weak,
	},
	refs::Reference,
	signature::{JavaType, MethodSignature, Primitive, ReturnType},
	strings::{JNIStr, JNIString},
	sys::{jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort},
};

mod util;
use util::{attach_current_thread, unwrap};

use rusty_fork::rusty_fork_test;

use crate::util::jvm;

static ARRAYLIST_CLASS: &JNIStr = jni_str!("java/util/ArrayList");
static EXCEPTION_CLASS: &JNIStr = jni_str!("java/lang/Exception");
static ARITHMETIC_EXCEPTION_CLASS: &JNIStr = jni_str!("java/lang/ArithmeticException");
static RUNTIME_EXCEPTION_CLASS: &JNIStr = jni_str!("java/lang/RuntimeException");
static INTEGER_CLASS: &JNIStr = jni_str!("java/lang/Integer");
static MATH_CLASS: &JNIStr = jni_str!("java/lang/Math");
static STRING_CLASS: &JNIStr = jni_str!("java/lang/String");
static MATH_ABS_METHOD_NAME: &JNIStr = jni_str!("abs");
static MATH_TO_INT_METHOD_NAME: &JNIStr = jni_str!("toIntExact");
static MATH_ABS_SIGNATURE: &MethodSignature = &jni_sig!("(I)I");
static MATH_TO_INT_SIGNATURE: &MethodSignature = &jni_sig!("(J)I");
static TEST_EXCEPTION_MESSAGE: &JNIStr = jni_str!("Default exception thrown");
static TESTING_OBJECT_STR: &JNIStr = jni_str!("TESTING OBJECT");

#[path = "jni_api/arrays.rs"]
mod arrays;
#[path = "jni_api/buffers.rs"]
mod buffers;
#[path = "jni_api/calls.rs"]
mod calls;
#[path = "jni_api/characters.rs"]
mod characters;
#[path = "jni_api/exception_assertions.rs"]
mod exception_assertions;
#[path = "jni_api/exceptions.rs"]
mod exceptions;
#[path = "jni_api/fields.rs"]
mod fields;
#[path = "jni_api/identity.rs"]
mod identity;
#[path = "jni_api/references.rs"]
mod references;

#[path = "jni_api/shared_env.rs"]
mod shared_env;

use exception_assertions::*;
