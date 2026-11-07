//! Rust bindings and utilities for the Java Native Interface.
#![allow(unsafe_code, reason = "JNI bindings require raw FFI operations.")]

extern crate self as abos_jni;

#[macro_use]
mod internal;
mod api;

pub use api::*;

#[cfg(windows)]
use internal::windows_sys;
