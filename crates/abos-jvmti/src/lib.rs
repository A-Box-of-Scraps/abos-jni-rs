//! Utilities for JVMTI 1.2-or-newer agents (Java 8+), interoperable with [`abos_jni`].
//!
//! Default constructors request the 1.2 baseline once; explicit-version constructors
//! can require newer interfaces. [`Env::version`] retains the actual version without
//! a native call. Newer callbacks, events, and capabilities are version-checked,
//! but availability and execution-context requirements still come from the VM.
//!
//! [`Env`] is a borrowed, cross-thread environment handle, not a JNI attachment
//! guard. JVM calls remain unsafe: callers must satisfy their JVMTI phase,
//! callback-context, and thread-attachment requirements. Environments are not
//! automatically disposed, since disposal invalidates callbacks and other handles.
#![allow(unsafe_code, reason = "JVMTI utilities invoke the native function table.")]

mod api;
mod internal;

pub use abos_jvmti_sys as sys;
pub use api::*;
