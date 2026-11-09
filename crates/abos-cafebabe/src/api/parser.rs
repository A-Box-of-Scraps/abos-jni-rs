use crate::{ClassFile, Result, internal::parser};

/// Extract metadata from a complete class file.
///
/// # Examples
///
/// ```no_run
/// use abos_cafebabe::{AccessFlags, parse_class};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let bytes = std::fs::read("Example.class")?;
/// let class = parse_class(&bytes)?;
///
/// println!("{}", class.this_class);
/// for method in &class.methods {
///     if method.access_flags.contains(AccessFlags::NATIVE) {
///         println!("{}{}", method.name, method.descriptor);
///     }
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Returns an error for unsupported versions, unknown constant-pool tags,
/// truncated structures, invalid metadata references or strings, or trailing bytes.
/// Attribute payloads and unused constants are not semantically validated.
pub fn parse_class(bytes: &[u8]) -> Result<ClassFile> {
	parser::parse_class(bytes)
}
