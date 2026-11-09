#![doc = include_str!("../README.md")]
#![no_std]
#![allow(missing_docs, reason = "Raw declarations mirror jvmti.h.")]
#![allow(non_snake_case, non_camel_case_types, reason = "Names match jvmti.h.")]
#![allow(
	clippy::wildcard_imports,
	reason = "Generated sibling modules share JNI and JVMTI types."
)]

mod api;
mod internal;

pub use abos_jni_sys::*;
pub use api::*;
