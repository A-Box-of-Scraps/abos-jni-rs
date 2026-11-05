use core::ffi::{c_char, c_void};

use abos_jni_sys_macros::jni_to_union;

use super::{JavaVM, jboolean, jint, jobject, jsize};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JavaVMOption {
	pub optionString: *mut c_char,
	pub extraInfo: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JavaVMInitArgs {
	pub version: jint,
	pub nOptions: jint,
	pub options: *mut JavaVMOption,
	pub ignoreUnrecognized: jboolean,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JavaVMAttachArgs {
	pub version: jint,
	pub name: *mut c_char,
	pub group: jobject,
}

#[repr(C)]
#[jni_to_union]
#[non_exhaustive]
#[derive(Copy, Clone)]
pub struct JNIInvokeInterface_ {
	#[jni_added("reserved")]
	pub reserved0: *mut c_void,
	#[jni_added("reserved")]
	pub reserved1: *mut c_void,
	#[jni_added("reserved")]
	pub reserved2: *mut c_void,
	pub DestroyJavaVM: unsafe extern "system" fn(vm: *mut JavaVM) -> jint,
	pub AttachCurrentThread:
		unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, args: *mut c_void) -> jint,

	pub DetachCurrentThread: unsafe extern "system" fn(vm: *mut JavaVM) -> jint,

	#[jni_added("1.2")]
	pub GetEnv: unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, version: jint) -> jint,

	#[jni_added("1.4")]
	pub AttachCurrentThreadAsDaemon:
		unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, args: *mut c_void) -> jint,
}

#[allow(unsafe_code, reason = "JNI invocation entry points are provided by the JVM.")]
unsafe extern "system" {
	pub fn JNI_GetDefaultJavaVMInitArgs(args: *mut c_void) -> jint;
	pub fn JNI_CreateJavaVM(pvm: *mut *mut JavaVM, penv: *mut *mut c_void, args: *mut c_void) -> jint;
	pub fn JNI_GetCreatedJavaVMs(vmBuf: *mut *mut JavaVM, bufLen: jsize, nVMs: *mut jsize) -> jint;
}
