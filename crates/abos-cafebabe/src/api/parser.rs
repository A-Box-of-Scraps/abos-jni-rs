use crate::{ClassFile, Result, internal::parser};

/// Extract metadata from a complete class file.
///
/// # Errors
/// Returns an error for unsupported versions, unknown constant-pool tags,
/// truncated structures, invalid metadata references or strings, or trailing bytes.
/// Attribute payloads and unused constants are not semantically validated.
pub fn parse_class(bytes: &[u8]) -> Result<ClassFile> {
	parser::parse_class(bytes)
}
