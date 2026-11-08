use core::ffi::{c_char, c_void};

use super::{JNIInvokeInterface_, JNINativeInterface_};

/// C variable arguments, including target-specific layout and parameter passing.
pub type va_list<'a> = core::ffi::VaList<'a>;

pub type jint = i32;
pub type jlong = i64;
pub type jbyte = i8;
/// JNI unsigned-byte boolean. Pass `JNI_FALSE` or `JNI_TRUE` to Java.
pub type jboolean = u8;
pub type jchar = u16;
pub type jshort = i16;
pub type jfloat = f32;
pub type jdouble = f64;
pub type jsize = jint;

#[derive(Debug)]
pub enum _jobject {}
pub type jobject = *mut _jobject;
pub type jclass = jobject;
pub type jthrowable = jobject;
pub type jstring = jobject;
pub type jarray = jobject;
pub type jbooleanArray = jarray;
pub type jbyteArray = jarray;
pub type jcharArray = jarray;
pub type jshortArray = jarray;
pub type jintArray = jarray;
pub type jlongArray = jarray;
pub type jfloatArray = jarray;
pub type jdoubleArray = jarray;
pub type jobjectArray = jarray;
pub type jweak = jobject;

#[repr(C)]
#[derive(Copy)]
pub union jvalue {
	pub z: jboolean,
	pub b: jbyte,
	pub c: jchar,
	pub s: jshort,
	pub i: jint,
	pub j: jlong,
	pub f: jfloat,
	pub d: jdouble,
	pub l: jobject,
}

#[derive(Debug)]
pub enum _jfieldID {}
pub type jfieldID = *mut _jfieldID;
#[derive(Debug)]
pub enum _jmethodID {}
pub type jmethodID = *mut _jmethodID;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub enum jobjectRefType {
	JNIInvalidRefType = 0,
	JNILocalRefType = 1,
	JNIGlobalRefType = 2,
	JNIWeakGlobalRefType = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JNINativeMethod {
	pub name: *mut c_char,
	pub signature: *mut c_char,
	pub fnPtr: *mut c_void,
}

pub type JNIEnv = *const JNINativeInterface_;
pub type JavaVM = *const JNIInvokeInterface_;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JNIEnv_ {
	pub functions: *const JNINativeInterface_,
}
