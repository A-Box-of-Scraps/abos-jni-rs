#![doc = include_str!("../README.md")]
#![allow(missing_docs, reason = "Raw JNI declarations mirror jni.h.")]
#![allow(non_snake_case, non_camel_case_types, reason = "Names match jni.h.")]
#![warn(rust_2018_idioms, missing_debug_implementations)]
#![no_std]

mod api;
mod internal;

pub use api::*;
