//! Compile Java sources from Cargo build scripts or Rust applications.
#![deny(missing_docs)]

#[doc = include_str!("../CHANGELOG.md")]
#[doc = "\n[Unreleased]: #unreleased\n"]
pub mod changelog {}

mod api;
mod internal;

pub use api::{Build, Error, JavaCompiler, Result};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
