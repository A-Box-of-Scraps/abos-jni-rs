//! Compile Java sources from Cargo build scripts or Rust applications.
#![deny(missing_docs)]

mod api;
mod internal;

pub use api::{Build, Error, JavaCompiler, Result};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
