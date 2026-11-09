//! Dependency-free Java class-file metadata extraction through Java 27 (major 71).
//!
//! Reads names, descriptors, inheritance and access flags. Attribute payloads,
//! including method bodies, are skipped without interpretation. This is not a
//! bytecode verifier: unused constant-pool references and descriptor semantics
//! are not validated. Modified UTF-8 is decoded only for extracted metadata;
//! metadata containing unpaired surrogates cannot be represented as Rust strings
//! and is rejected. Unused Java string constants may contain unpaired surrogates.

#[doc = include_str!("../CHANGELOG.md")]
#[doc = "\n[Unreleased]: #unreleased\n"]
pub mod changelog {}

mod api;
mod internal;

pub use api::{AccessFlags, ClassFile, Member, ParseError, parse_class};

type Result<T> = std::result::Result<T, ParseError>;
