#![allow(missing_docs, reason = "Raw JNI declarations mirror jni.h.")]
#![allow(non_snake_case, non_camel_case_types, reason = "Names match jni.h.")]
#![warn(rust_2018_idioms, missing_debug_implementations)]
#![no_std]

#[doc = include_str!("../CHANGELOG.md")]
#[doc = "\n\n[Unreleased]: #unreleased\n"]
pub mod changelog {}

mod api;
mod internal;

pub use api::*;
