/// Access flags shared by the class, field and method metadata.
///
/// Only selected flags have named constants. All bits are preserved in the public
/// tuple field (`flags.0`), including flags without a named constant. Interpret
/// those bits according to whether the flags belong to a class, field or method.
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
