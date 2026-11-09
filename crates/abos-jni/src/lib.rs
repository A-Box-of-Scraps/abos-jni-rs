//! Rust bindings and utilities for the Java Native Interface.
#![allow(unsafe_code, reason = "JNI bindings require raw FFI operations.")]

#[doc = include_str!("../CHANGELOG.md")]
#[doc = "\n[Unreleased]: #unreleased\n"]
pub mod changelog {}

extern crate self as abos_jni;

#[macro_use]
mod internal;
mod api;

pub use api::*;

#[cfg(windows)]
use internal::windows_sys;
