#![doc = include_str!("../README.md")]
#![allow(unsafe_code, reason = "JVMTI utilities invoke the native function table.")]

mod api;
mod internal;

pub use abos_jvmti_sys as sys;
pub use api::*;
