#![allow(missing_docs, reason = "Integration test crate with no public API.")]

use std::{
	ffi::c_void,
	mem::{align_of, offset_of, size_of},
};

use abos_jni_sys as sys;
use sys::{
	JNIEnv, JNIInvokeInterface_, JNIInvokeInterface__1_1, JNIInvokeInterface__1_2, JNIInvokeInterface__1_4,
	JNINativeInterface_, JNINativeInterface__1_1, JNINativeInterface__1_2, JNINativeInterface__1_4,
	JNINativeInterface__1_6, JNINativeInterface__9, JNINativeInterface__19, JNINativeInterface__24,
	JNINativeInterface__reserved, JavaVM, jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jvalue,
};

const SLOT: usize = size_of::<*mut c_void>();

#[test]
fn native_table_version_sizes() {
	assert_eq!(size_of::<JNINativeInterface_>(), 236 * SLOT);
	assert_eq!(align_of::<JNINativeInterface_>(), align_of::<*mut c_void>());
	assert_eq!(size_of::<JNINativeInterface__1_1>(), 220 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__1_2>(), 229 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__1_4>(), 232 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__1_6>(), 233 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__9>(), 234 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__19>(), 235 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__24>(), 236 * SLOT);
	assert_eq!(size_of::<JNINativeInterface__reserved>(), 236 * SLOT);
}

#[test]
fn native_table_versions_preserve_slot_offsets() {
	assert_eq!(offset_of!(JNINativeInterface__1_1, GetVersion), 4 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__1_1, GetSuperclass), 10 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__1_2, FromReflectedMethod), 7 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__1_2, ExceptionCheck), 228 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__1_4, NewDirectByteBuffer), 229 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__1_6, GetObjectRefType), 232 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__9, GetModule), 233 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__19, IsVirtualThread), 234 * SLOT);
	assert_eq!(offset_of!(JNINativeInterface__24, GetStringUTFLengthAsLong), 235 * SLOT);
}

#[test]
fn invocation_table_versions_preserve_slot_offsets() {
	assert_eq!(size_of::<JNIInvokeInterface_>(), 8 * SLOT);
	assert_eq!(size_of::<JNIInvokeInterface__1_1>(), 6 * SLOT);
	assert_eq!(size_of::<JNIInvokeInterface__1_2>(), 7 * SLOT);
	assert_eq!(size_of::<JNIInvokeInterface__1_4>(), 8 * SLOT);
	assert_eq!(offset_of!(JNIInvokeInterface__1_1, DestroyJavaVM), 3 * SLOT);
	assert_eq!(offset_of!(JNIInvokeInterface__1_2, GetEnv), 6 * SLOT);
	assert_eq!(
		offset_of!(JNIInvokeInterface__1_4, AttachCurrentThreadAsDaemon),
		7 * SLOT
	);
}

#[test]
fn scalar_and_value_layouts() {
	assert_eq!(size_of::<jboolean>(), 1);
	assert_eq!(size_of::<jbyte>(), 1);
	assert_eq!(size_of::<jchar>(), 2);
	assert_eq!(size_of::<jshort>(), 2);
	assert_eq!(size_of::<jint>(), 4);
	assert_eq!(size_of::<jlong>(), 8);
	assert_eq!(size_of::<jfloat>(), 4);
	assert_eq!(size_of::<jdouble>(), 8);
	assert_eq!(size_of::<jvalue>(), size_of::<jlong>().max(SLOT));
	assert_eq!(size_of::<JNIEnv>(), SLOT);
	assert_eq!(size_of::<JavaVM>(), SLOT);
}

#[test]
fn debug_does_not_read_inactive_union_members() {
	let value = jvalue { b: -1 };
	assert_eq!(format!("{value:?}"), "jvalue { .. }");
}

#[test]
fn unavailable_fields_are_rejected() {
	let cases = trybuild::TestCases::new();
	cases.compile_fail("tests/trybuild/*.rs");
}
