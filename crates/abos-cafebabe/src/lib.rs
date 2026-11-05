//! Dependency-free Java class-file metadata extraction through Java 27 (major 71).
//!
//! Reads names, descriptors, inheritance and access flags. Attribute payloads,
//! including method bodies, are skipped without interpretation. This is not a
//! bytecode verifier: unused constant-pool references and descriptor semantics
//! are not validated. Modified UTF-8 is decoded only for extracted metadata;
//! metadata containing unpaired surrogates cannot be represented as Rust strings
//! and is rejected. Unused Java string constants may contain unpaired surrogates.

mod pool;
mod reader;

use pool::Pool;
use reader::Reader;

/// A class-file parsing error with a byte offset into the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
	/// Position at which parsing failed.
	pub offset: usize,
	/// Description of the invalid or unsupported structure.
	pub message: &'static str,
}

impl std::fmt::Display for ParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} at byte {}", self.message, self.offset)
	}
}

impl std::error::Error for ParseError {}

type Result<T> = std::result::Result<T, ParseError>;

/// Access flags shared by the class, field and method metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessFlags(pub u16);

impl AccessFlags {
	/// Public visibility.
	pub const PUBLIC: Self = Self(0x0001);
	/// Private visibility.
	pub const PRIVATE: Self = Self(0x0002);
	/// Static member.
	pub const STATIC: Self = Self(0x0008);
	/// Final class or member.
	pub const FINAL: Self = Self(0x0010);
	/// Native method.
	pub const NATIVE: Self = Self(0x0100);

	/// Whether all supplied bits are set.
	#[must_use]
	pub const fn contains(self, flags: Self) -> bool {
		self.0 & flags.0 == flags.0
	}
}

/// A field or method declaration, without attributes or executable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
	/// Unqualified member name.
	pub name: String,
	/// JVM field or method descriptor, not a generic signature.
	pub descriptor: String,
	/// Raw declaration flags.
	pub access_flags: AccessFlags,
}

/// Owned metadata extracted from one class file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassFile {
	/// Class-file major version (45 through 71).
	pub major_version: u16,
	/// Class-file minor version, including 65535 for preview classes.
	pub minor_version: u16,
	/// Raw class flags.
	pub access_flags: AccessFlags,
	/// Internal class name, with slash-separated packages.
	pub this_class: String,
	/// Internal superclass name, absent for Object and module descriptors.
	pub super_class: Option<String>,
	/// Internal names of directly implemented interfaces.
	pub interfaces: Vec<String>,
	/// Declared fields, in class-file order.
	pub fields: Vec<Member>,
	/// Declared methods, including constructors and class initializers.
	pub methods: Vec<Member>,
}

/// Extract metadata from a complete class file.
///
/// # Errors
/// Returns an error for unsupported versions, unknown constant-pool tags,
/// truncated structures, invalid metadata references or strings, or trailing bytes.
/// Attribute payloads and unused constants are not semantically validated.
pub fn parse_class(bytes: &[u8]) -> Result<ClassFile> {
	let mut reader: Reader<'_> = Reader::new(bytes);
	if reader.u32()? != 0xCAFE_BABE {
		return Err(reader.error("invalid class-file magic"));
	}
	let minor_version: u16 = reader.u16()?;
	let major_version: u16 = reader.u16()?;
	if !(45..=71).contains(&major_version) {
		return Err(reader.error("unsupported class-file major version (expected 45..=71)"));
	}
	if major_version >= 56 && minor_version != 0 && minor_version != u16::MAX {
		return Err(reader.error("invalid class-file minor version"));
	}
	let pool: Pool<'_> = Pool::read(&mut reader)?;
	let access_flags: AccessFlags = AccessFlags(reader.u16()?);
	let this_class: String = pool.class_name(&mut reader)?;
	let parent_index: u16 = reader.u16()?;
	let super_class: Option<String> = if parent_index == 0 {
		None
	} else {
		Some(pool.class(parent_index, &reader)?)
	};
	let mut interfaces: Vec<String> = Vec::new();
	for _ in 0..reader.u16()? {
		interfaces.push(pool.class_name(&mut reader)?);
	}
	let fields: Vec<Member> = read_members(&mut reader, &pool)?;
	let methods: Vec<Member> = read_members(&mut reader, &pool)?;
	skip_attributes(&mut reader, &pool)?;
	if !reader.is_empty() {
		return Err(reader.error("trailing bytes after class file"));
	}
	Ok(ClassFile {
		major_version,
		minor_version,
		access_flags,
		this_class,
		super_class,
		interfaces,
		fields,
		methods,
	})
}

fn read_members(reader: &mut Reader<'_>, pool: &Pool<'_>) -> Result<Vec<Member>> {
	let mut members: Vec<Member> = Vec::new();
	for _ in 0..reader.u16()? {
		let access_flags: AccessFlags = AccessFlags(reader.u16()?);
		let name: String = pool.text(reader.u16()?, reader)?;
		let descriptor: String = pool.text(reader.u16()?, reader)?;
		skip_attributes(reader, pool)?;
		members.push(Member {
			name,
			descriptor,
			access_flags,
		});
	}
	Ok(members)
}

fn skip_attributes(reader: &mut Reader<'_>, pool: &Pool<'_>) -> Result<()> {
	for _ in 0..reader.u16()? {
		// Attribute names must reference UTF-8 constants, but their payloads are opaque.
		pool.utf8(reader.u16()?, reader)?;
		let length: usize =
			usize::try_from(reader.u32()?).map_err(|_| reader.error("attribute length exceeds address space"))?;
		reader.take(length)?;
	}
	Ok(())
}
