use std::{fmt, str::FromStr};

use crate::{
	errors::{Error, Result},
	strings::{JNIStr, JNIString},
};

#[cfg(doc)]
use crate::{Env, jni_sig};

/// A primitive java type. These are the things that can be represented without
/// an object.
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum Primitive {
	Boolean, // Z
	Byte,    // B
	Char,    // C
	Double,  // D
	Float,   // F
	Int,     // I
	Long,    // J
	Short,   // S
	Void,    // V
}

impl fmt::Display for Primitive {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Self::Boolean => write!(f, "Z"),
			Self::Byte => write!(f, "B"),
			Self::Char => write!(f, "C"),
			Self::Double => write!(f, "D"),
			Self::Float => write!(f, "F"),
			Self::Int => write!(f, "I"),
			Self::Long => write!(f, "J"),
			Self::Short => write!(f, "S"),
			Self::Void => write!(f, "V"),
		}
	}
}

/// Enum representing any java type
///
/// This intentionally does not keep track of the object class names or details of array elements
/// since there would be a cost to tracking those strings and handling variable array dimensions
/// while JNI generally only needs to differentiate between primitive types and reference types.
///
/// Use [`MethodSignature::sig`] or [`FieldSignature::sig`] for the complete descriptor,
/// including class names and array dimensions. `Display` only emits a category marker
/// for references (`L;` or `[`), not a complete object or array descriptor.
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum JavaType {
	Primitive(Primitive),
	Object,
	Array,
}

impl FromStr for JavaType {
	type Err = Error;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		let (ty, rest) = parse_type(s)?;
		match rest {
			"" => Ok(ty),
			_ => Err(Error::ParseFailed(format!(
				"Trailing input: '{rest}' while parsing '{s}'"
			))),
		}
	}
}

impl fmt::Display for JavaType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Self::Primitive(ref ty) => ty.fmt(f),
			Self::Object => write!(f, "L;"),
			Self::Array => write!(f, "["),
		}
	}
}

/// Enum representing any java type that may be used as a return value
pub type ReturnType = JavaType;

/// A parsed JNI method signature
///
/// This is a structured representation of a JNI method signature, such as
/// `(Ljava/lang/String;)Z`.
///
/// The decomposed JNI value categories are guaranteed to match the signature string.
/// Reference arguments still require runtime assignability checks against the resolved member.
///
/// Most of the time you should use the [`jni_sig!`] macro to derive method
/// signatures at compile time.
///
/// If you need to parse method signatures at runtime, use
/// [`RuntimeMethodSignature::from_str`] followed by
/// [`RuntimeMethodSignature::method_signature`].
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct MethodSignature<'sig, 'args> {
	sig: &'sig JNIStr,
	args: &'args [JavaType],
	ret: JavaType,
}

impl<'sig, 'args> MethodSignature<'sig, 'args> {
	/// Create a `MethodSignature` from its raw parts
	///
	/// # Safety
	///
	/// In order for the returned `MethodSignature` to be used safely to make
	/// JNI calls, the caller must ensure that the provided signature string,
	/// argument types, and return type are consistent
	#[must_use]
	pub const unsafe fn from_raw_parts(sig: &'sig JNIStr, args: &'args [JavaType], ret: ReturnType) -> Self {
		Self { sig, args, ret }
	}

	/// Get the JNI signature string
	#[must_use]
	pub const fn sig(&self) -> &JNIStr {
		self.sig
	}

	/// Get the argument types
	#[must_use]
	pub const fn args(&self) -> &[JavaType] {
		self.args
	}

	/// Get the return type
	#[must_use]
	pub const fn ret(&self) -> JavaType {
		self.ret
	}
}

impl From<&Self> for MethodSignature<'_, '_> {
	fn from(sig: &Self) -> Self {
		sig.clone()
	}
}

/// A parsed JNI field signature
///
/// This is a structured representation of a JNI field signature, such
/// as `I`.
///
/// The field's JNI value category is guaranteed to match the signature string.
/// Reference values still require runtime assignability checks against the resolved field.
///
/// Most of the time you should use the [`jni_sig!`] macro to derive field
/// signatures at compile time.
///
/// If you need to parse field signatures at runtime, use
/// [`RuntimeFieldSignature::from_str`] followed by
/// [`RuntimeFieldSignature::field_signature`].
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct FieldSignature<'sig> {
	sig: &'sig JNIStr,
	ty: JavaType,
}

impl<'sig> FieldSignature<'sig> {
	/// Create a `FieldSignature` from its raw parts
	///
	/// # Safety
	///
	/// In order for the returned `FieldSignature` to be used safely to get or
	/// set fields via JNI calls, the caller must ensure that the provided
	/// signature string and field type are consistent
	#[must_use]
	pub const unsafe fn from_raw_parts(sig: &'sig JNIStr, ty: JavaType) -> Self {
		Self { sig, ty }
	}

	/// Get the JNI signature string
	#[must_use]
	pub const fn sig(&self) -> &JNIStr {
		self.sig
	}

	/// Get the field type
	#[must_use]
	pub const fn ty(&self) -> JavaType {
		self.ty
	}

	/// Check whether the JNI value categories are compatible. This
	/// is not quite an equality check on `Self::ty()` because `JavaType::Array`
	/// signatures are considered compatible with `JavaType::Object` values.
	///
	/// This does not validate reference assignability: `JavaType` does not retain class names
	/// or array component types. Safe field setters additionally check the actual value
	/// against the resolved field's runtime class.
	#[must_use]
	pub fn type_is_compatible_with(&self, ty: JavaType) -> bool {
		match (self.ty, ty) {
			(JavaType::Array | JavaType::Object, JavaType::Array | JavaType::Object) => true,
			(a, b) if a == b => true,
			_ => false,
		}
	}
}

/// A runtime-parsed JNI method signature.
///
/// This is a structured representation of a JNI method signature, such
/// as `(Ljava/lang/String;)Z`.
///
/// The decomposed JNI value categories are guaranteed to match the signature string.
/// Reference arguments still require runtime assignability checks against the resolved member.
///
/// A reference to `RuntimeSignature` can be converted into a `Signature`, which
/// is used by the JNI functions, such as `call_(object|static)_method`.
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RuntimeMethodSignature {
	sig: JNIString,
	args: Vec<JavaType>,
	ret: JavaType,
}

impl AsRef<Self> for MethodSignature<'_, '_> {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<'a> From<&'a RuntimeMethodSignature> for MethodSignature<'a, 'a> {
	fn from(sig: &'a RuntimeMethodSignature) -> Self {
		Self {
			sig: &sig.sig,
			args: &sig.args,
			ret: sig.ret,
		}
	}
}

impl<'sig, 'args, M> From<M> for RuntimeMethodSignature
where
	M: AsRef<MethodSignature<'sig, 'args>>,
{
	fn from(sig: M) -> Self {
		let sig: &MethodSignature<'_, '_> = sig.as_ref();
		Self {
			sig: sig.sig().into(),
			args: sig.args().to_vec(),
			ret: sig.ret(),
		}
	}
}

impl RuntimeMethodSignature {
	/// Parse a method signature string into a `RuntimeMethodSignature` enum.
	#[allow(
		clippy::should_implement_trait,
		reason = "This existing parser accepts any AsRef<str>, unlike FromStr::from_str."
	)]
	///
	/// # Errors
	///
	/// Returns a parse error if the input is not a complete, valid JNI signature.
	pub fn from_str<S: AsRef<str>>(s: S) -> Result<Self> {
		let input: &str = s.as_ref();
		match parse_method_sig(input)? {
			// Note: the parser initially returns a placeholder, empty signature string,
			(Self { sig: _, args, ret }, "") => Ok(Self {
				sig: JNIString::new(input),
				args,
				ret,
			}),
			(Self { .. }, tail) => Err(Error::ParseFailed(format!(
				"Trailing input: '{tail}' while parsing '{input}'"
			))),
		}
	}

	/// Convert to a [`MethodSignature`] (for use with [Env] calls like [`Env::call_method`]).
	///
	/// This is a cheap conversion, which borrows the signature string and
	/// slices the argument vector, without any allocations.
	#[must_use]
	pub fn method_signature(&self) -> MethodSignature<'_, '_> {
		self.into()
	}
}

impl FromStr for RuntimeMethodSignature {
	type Err = Error;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		Self::from_str(s)
	}
}

impl fmt::Display for RuntimeMethodSignature {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "(")?;
		for a in &self.args {
			write!(f, "{a}")?;
		}
		write!(f, ")")?;
		write!(f, "{}", self.ret)?;
		Ok(())
	}
}

/// A runtime-parsed JNI field signature.
///
/// This is a structured representation of a JNI field signature, such as `[Ljava/lang/String;`.
///
/// The field's JNI value category is guaranteed to match the signature string.
/// Reference values still require runtime assignability checks against the resolved field.
///
/// A reference to `RuntimeFieldSignature` can be converted into a
/// `FieldSignature`, which is used by the JNI functions, such as
/// `(get|set)_(static)_field`.
#[allow(
	missing_docs,
	reason = "Signature representations expose JNI type names and their component data."
)]
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RuntimeFieldSignature {
	sig: JNIString,
	ty: JavaType,
}

impl AsRef<Self> for FieldSignature<'_> {
	fn as_ref(&self) -> &Self {
		self
	}
}
impl<'a> From<&'a RuntimeFieldSignature> for FieldSignature<'a> {
	fn from(sig: &'a RuntimeFieldSignature) -> Self {
		Self {
			sig: &sig.sig,
			ty: sig.ty,
		}
	}
}

impl<'sig, F> From<F> for RuntimeFieldSignature
where
	F: AsRef<FieldSignature<'sig>>,
{
	fn from(sig: F) -> Self {
		let sig: &FieldSignature<'_> = sig.as_ref();
		Self {
			sig: sig.sig().into(),
			ty: sig.ty(),
		}
	}
}

impl RuntimeFieldSignature {
	/// Parse a field signature string into a `RuntimeFieldSignature`.
	#[allow(
		clippy::should_implement_trait,
		reason = "This existing parser accepts any AsRef<str>, unlike FromStr::from_str."
	)]
	///
	/// # Errors
	///
	/// Returns a parse error if the input is not a complete, valid JNI signature.
	pub fn from_str<S: AsRef<str>>(s: S) -> Result<Self> {
		let input: &str = s.as_ref();
		match parse_field_sig(input)? {
			// Note: the parser initially returns a placeholder, empty signature string,
			(Self { sig: _, ty }, "") => Ok(Self {
				sig: JNIString::new(input),
				ty,
			}),
			(Self { .. }, tail) => Err(Error::ParseFailed(format!(
				"Trailing input: '{tail}' while parsing '{input}'"
			))),
		}
	}

	/// Convert to a [`FieldSignature`] (for use with [Env] calls like [`Env::get_field`]).
	///
	/// This is a cheap conversion, which borrows the signature string and
	/// slices the argument vector, without any allocations.
	#[must_use]
	pub fn field_signature(&self) -> FieldSignature<'_> {
		self.into()
	}
}

impl FromStr for RuntimeFieldSignature {
	type Err = Error;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		Self::from_str(s)
	}
}

impl fmt::Display for RuntimeFieldSignature {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.ty)
	}
}

mod parser;
use parser::{parse_field_sig, parse_method_sig, parse_type};
#[cfg(test)]
#[path = "../../../tests/unit/api/signature.rs"]
mod tests;
