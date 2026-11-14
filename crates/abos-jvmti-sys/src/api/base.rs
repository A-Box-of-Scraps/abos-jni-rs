use super::*;

pub type jthread = jobject;
pub type jthreadGroup = jobject;
pub type jlocation = jlong;

#[derive(Debug)]
#[allow(
	clippy::empty_enums,
	reason = "Opaque FFI handle tags must be distinct and cannot be instantiated."
)]
pub enum _jrawMonitorID {}
pub type jrawMonitorID = *mut _jrawMonitorID;
pub type jniNativeInterface = JNINativeInterface_;
pub type jvmtiEnv = *const jvmtiInterface_1_;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct _jvmtiEnv {
	pub functions: *const jvmtiInterface_1_,
}
