//! Raw, `no_std` bindings to the JDK 25 Java Virtual Machine Tool Interface.
//!
//! JNI types are shared with [`abos_jni_sys`]. Function pointers and callbacks
//! are nullable. Reserved slots must not be called. Obtain an environment using
//! `JavaVM::GetEnv` with [`JVMTI_VERSION`]; older environments need not support
//! every function or event represented here. Historical function-table views start
//! at [`jvmtiInterface_1__1_2`]; only access a view after establishing its required
//! version. Never form a full JDK 25 table reference over a shorter older table.
//!
//! These declarations do not enforce VM phases, capabilities, callback safety,
//! thread attachment, or the lifetimes of environments and JNI references.
//! Capability bitfields follow the platform C ABI; the standalone header probe
//! verifies their layout against the installed JDK.
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
