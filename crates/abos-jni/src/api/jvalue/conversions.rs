use super::*;

impl<'obj_ref> From<&'obj_ref JValueOwned<'obj_ref>> for JValue<'obj_ref> {
	fn from(other: &'obj_ref JValueOwned) -> Self {
		other.borrow()
	}
}

impl<'local, T: Into<JObject<'local>>> From<T> for JValueOwned<'local> {
	fn from(other: T) -> Self {
		Self::Object(other.into())
	}
}

impl<'obj_ref, T: AsRef<JObject<'obj_ref>>> From<&'obj_ref T> for JValue<'obj_ref> {
	fn from(other: &'obj_ref T) -> Self {
		Self::Object(other.as_ref())
	}
}

impl<'local> TryFrom<JValueOwned<'local>> for JObject<'local> {
	type Error = Error;

	fn try_from(value: JValueOwned<'local>) -> Result<Self> {
		value.l()
	}
}

impl From<jboolean> for JValueOwned<'_> {
	fn from(other: jboolean) -> Self {
		Self::Bool(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jboolean {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.borrow().try_into()
	}
}

impl From<jboolean> for JValue<'_> {
	fn from(other: jboolean) -> Self {
		Self::Bool(other)
	}
}

impl TryFrom<JValue<'_>> for jboolean {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		match value {
			JValue::Bool(b) => Ok(b),
			_ => Err(Error::WrongJValueType("bool", value.type_name())),
		}
	}
}

// jchar
impl From<jchar> for JValueOwned<'_> {
	fn from(other: jchar) -> Self {
		Self::Char(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jchar {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.c()
	}
}

impl From<jchar> for JValue<'_> {
	fn from(other: jchar) -> Self {
		Self::Char(other)
	}
}

impl TryFrom<JValue<'_>> for jchar {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.c()
	}
}

/// Converts a Rust `char` to a Java `char`, if possible.
///
/// **Warning:** This conversion is likely to fail. Using it is not recommended. Prefer
/// [`JValue::int_from_char`] where possible. See [`char_to_java`] for more information.
impl TryFrom<char> for JValueOwned<'_> {
	type Error = CharToJavaError;

	fn try_from(value: char) -> std::result::Result<Self, Self::Error> {
		Ok(Self::Char(char_to_java(value)?))
	}
}

/// Converts a Rust `char` to a Java `char`, if possible.
///
/// **Warning:** This conversion is likely to fail. Using it is not recommended. Prefer
/// [`JValue::int_from_char`] where possible. See [`char_to_java`] for more information.
impl TryFrom<char> for JValue<'_> {
	type Error = CharToJavaError;

	fn try_from(value: char) -> std::result::Result<Self, Self::Error> {
		Ok(Self::Char(char_to_java(value)?))
	}
}

impl From<jshort> for JValueOwned<'_> {
	fn from(other: jshort) -> Self {
		Self::Short(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jshort {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.s()
	}
}

impl From<jshort> for JValue<'_> {
	fn from(other: jshort) -> Self {
		Self::Short(other)
	}
}

impl TryFrom<JValue<'_>> for jshort {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.s()
	}
}

// jfloat
impl From<jfloat> for JValueOwned<'_> {
	fn from(other: jfloat) -> Self {
		Self::Float(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jfloat {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.f()
	}
}

impl From<jfloat> for JValue<'_> {
	fn from(other: jfloat) -> Self {
		Self::Float(other)
	}
}

impl TryFrom<JValue<'_>> for jfloat {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.f()
	}
}

// jdouble
impl From<jdouble> for JValueOwned<'_> {
	fn from(other: jdouble) -> Self {
		Self::Double(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jdouble {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.d()
	}
}

impl From<jdouble> for JValue<'_> {
	fn from(other: jdouble) -> Self {
		Self::Double(other)
	}
}

impl TryFrom<JValue<'_>> for jdouble {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.d()
	}
}

// jint
impl From<jint> for JValueOwned<'_> {
	fn from(other: jint) -> Self {
		Self::Int(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jint {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.i()
	}
}

impl From<jint> for JValue<'_> {
	fn from(other: jint) -> Self {
		Self::Int(other)
	}
}

impl TryFrom<JValue<'_>> for jint {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.i()
	}
}

impl From<jlong> for JValueOwned<'_> {
	fn from(other: jlong) -> Self {
		Self::Long(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jlong {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.j()
	}
}

impl From<jlong> for JValue<'_> {
	fn from(other: jlong) -> Self {
		Self::Long(other)
	}
}

impl TryFrom<JValue<'_>> for jlong {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.j()
	}
}

// jbyte
impl From<jbyte> for JValueOwned<'_> {
	fn from(other: jbyte) -> Self {
		Self::Byte(other)
	}
}

impl TryFrom<JValueOwned<'_>> for jbyte {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.b()
	}
}

impl From<jbyte> for JValue<'_> {
	fn from(other: jbyte) -> Self {
		Self::Byte(other)
	}
}

impl TryFrom<JValue<'_>> for jbyte {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.b()
	}
}

// jvoid
impl From<()> for JValueOwned<'_> {
	fn from(_: ()) -> Self {
		Self::Void
	}
}

impl TryFrom<JValueOwned<'_>> for () {
	type Error = Error;

	fn try_from(value: JValueOwned) -> Result<Self> {
		value.v()
	}
}

impl From<()> for JValue<'_> {
	fn from(_: ()) -> Self {
		Self::Void
	}
}

impl TryFrom<JValue<'_>> for () {
	type Error = Error;

	fn try_from(value: JValue) -> Result<Self> {
		value.v()
	}
}
