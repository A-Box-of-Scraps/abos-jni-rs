//! Utilities for JDK 25 JVMTI agents, interoperable with [`abos_jni`].
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
