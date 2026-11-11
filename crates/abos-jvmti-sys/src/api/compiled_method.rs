//! Implementation-specific `jvmticmlr.h` records, not part of standard `jvmti.h`.
//!
//! Only interpret `compile_info` this way when the VM implements this format.
//! All records and nested arrays are borrowed until `CompiledMethodLoad` returns.

#![allow(clippy::use_self, reason = "Keep C names visible for the header ABI probe.")]

use super::*;

pub const JVMTI_CMLR_MAJOR_VERSION_1: jint = 1;
pub const JVMTI_CMLR_MINOR_VERSION_0: jint = 0;
pub const JVMTI_CMLR_MAJOR_VERSION: jint = 1;
pub const JVMTI_CMLR_MINOR_VERSION: jint = 0;

// An integer, rather than a Rust enum, preserves unknown implementation kinds.
#[cfg(target_env = "msvc")]
pub type jvmtiCMLRKind = i32;
#[cfg(not(target_env = "msvc"))]
pub type jvmtiCMLRKind = u32;
pub const JVMTI_CMLR_DUMMY: jvmtiCMLRKind = 1;
pub const JVMTI_CMLR_INLINE_INFO: jvmtiCMLRKind = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct jvmtiCompiledMethodLoadRecordHeader {
	pub kind: jvmtiCMLRKind,
	pub majorinfoversion: jint,
	pub minorinfoversion: jint,
	pub next: *mut jvmtiCompiledMethodLoadRecordHeader,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PCStackInfo {
	pub pc: *mut c_void,
	pub numstackframes: jint,
	pub methods: *mut jmethodID,
	pub bcis: *mut jint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct jvmtiCompiledMethodLoadInlineRecord {
	pub header: jvmtiCompiledMethodLoadRecordHeader,
	pub numpcs: jint,
	pub pcinfo: *mut PCStackInfo,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct jvmtiCompiledMethodLoadDummyRecord {
	pub header: jvmtiCompiledMethodLoadRecordHeader,
	pub message: [c_char; 50],
}
