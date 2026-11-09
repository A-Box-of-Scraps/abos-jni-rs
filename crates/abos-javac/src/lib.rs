#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod api;
mod internal;

pub use api::{Build, Error, JavaCompiler, Result};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
