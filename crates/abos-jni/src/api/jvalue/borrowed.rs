use super::{
	Error, JNI_TRUE, JObject, JValue, JavaType, Primitive, Result, char_from_java, char_from_java_int,
	char_to_java_int, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jvalue, trace,
};

impl<'obj_ref> JValue<'obj_ref> {
	/// Convert the enum to its jni-compatible equivalent.
	#[must_use]
	pub fn as_jni(&self) -> jvalue {
		let val: jvalue = match self {
			Self::Object(obj) => jvalue { l: obj.as_raw() },
			Self::Byte(byte) => jvalue { b: *byte },
			Self::Char(char) => jvalue { c: *char },
			Self::Short(short) => jvalue { s: *short },
			Self::Int(int) => jvalue { i: *int },
			Self::Long(long) => jvalue { j: *long },
			Self::Bool(boolean) => jvalue { b: i8::from(*boolean) },
			Self::Float(float) => jvalue { f: *float },
			Self::Double(double) => jvalue { d: *double },
			Self::Void => jvalue {
				l: ::std::ptr::null_mut(),
			},
		};
		trace!("converted {:?} to jvalue {:?}", self, unsafe {
			::std::mem::transmute::<jvalue, u64>(val)
		});
		val
	}

	/// Convert the enum to its jni-compatible equivalent.
	#[deprecated = "Use `as_jni` instead."]
	#[must_use]
	pub fn to_jni(self) -> jvalue {
		self.as_jni()
	}

	/// Get the type name for the enum variant.
	#[must_use]
	pub const fn type_name(&self) -> &'static str {
		match *self {
			Self::Void => "void",
			Self::Object(_) => "object",
			Self::Byte(_) => "byte",
			Self::Char(_) => "char",
			Self::Short(_) => "short",
			Self::Int(_) => "int",
			Self::Long(_) => "long",
			Self::Bool(_) => "bool",
			Self::Float(_) => "float",
			Self::Double(_) => "double",
		}
	}

	/// Get the corresponding Java type.
	#[must_use]
	pub const fn java_type(&self) -> JavaType {
		match *self {
			Self::Object(_) => JavaType::Object,
			Self::Void => JavaType::Primitive(Primitive::Void),
			Self::Byte(_) => JavaType::Primitive(Primitive::Byte),
			Self::Char(_) => JavaType::Primitive(Primitive::Char),
			Self::Short(_) => JavaType::Primitive(Primitive::Short),
			Self::Int(_) => JavaType::Primitive(Primitive::Int),
			Self::Long(_) => JavaType::Primitive(Primitive::Long),
			Self::Bool(_) => JavaType::Primitive(Primitive::Boolean),
			Self::Float(_) => JavaType::Primitive(Primitive::Float),
			Self::Double(_) => JavaType::Primitive(Primitive::Double),
		}
	}

	/// Get the primitive type for the enum variant. If it's not a primitive
	/// (i.e. an Object), returns None.
	#[must_use]
	pub const fn primitive_type(&self) -> Option<Primitive> {
		Some(match *self {
			Self::Object(_) => return None,
			Self::Void => Primitive::Void,
			Self::Byte(_) => Primitive::Byte,
			Self::Char(_) => Primitive::Char,
			Self::Short(_) => Primitive::Short,
			Self::Int(_) => Primitive::Int,
			Self::Long(_) => Primitive::Long,
			Self::Bool(_) => Primitive::Boolean,
			Self::Float(_) => Primitive::Float,
			Self::Double(_) => Primitive::Double,
		})
	}

	/// Try to unwrap to an Object.
	pub const fn l(self) -> Result<&'obj_ref JObject<'obj_ref>> {
		match self {
			Self::Object(obj) => Ok(obj),
			_ => Err(Error::WrongJValueType("object", self.type_name())),
		}
	}

	/// Try to unwrap to a boolean.
	pub const fn z(self) -> Result<bool> {
		match self {
			Self::Bool(b) => Ok(b == JNI_TRUE),
			_ => Err(Error::WrongJValueType("bool", self.type_name())),
		}
	}

	/// Try to unwrap to a byte.
	pub const fn b(self) -> Result<jbyte> {
		match self {
			Self::Byte(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jbyte", self.type_name())),
		}
	}

	/// Try to unwrap to a char.
	pub const fn c(self) -> Result<jchar> {
		match self {
			Self::Char(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jchar", self.type_name())),
		}
	}

	/// Try to unwrap a Java `char` and then convert it to a Rust `char`.
	///
	/// **Warning:** This conversion is likely to fail. Using it is not recommended. Prefer
	/// [`JValue::i_char`] where possible. See [`char_from_java`] for more information.
	///
	/// # Errors
	///
	/// This method can fail with two kinds of errors:
	///
	/// * [`Error::WrongJValueType`]: `self` does not contain a Java `char`.
	/// * [`Error::InvalidUtf16`]: `self` contains a Java `char`, but it is one half of a surrogate
	///   pair.
	pub fn c_char(self) -> Result<char> {
		let char = self.c()?;

		char_from_java(char).map_err(|source| Error::InvalidUtf16 { source })
	}

	/// Try to unwrap to a double.
	pub const fn d(self) -> Result<jdouble> {
		match self {
			Self::Double(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jdouble", self.type_name())),
		}
	}

	/// Try to unwrap to a float.
	pub const fn f(self) -> Result<jfloat> {
		match self {
			Self::Float(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jfloat", self.type_name())),
		}
	}

	/// Try to unwrap to an int.
	pub const fn i(self) -> Result<jint> {
		match self {
			Self::Int(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jint", self.type_name())),
		}
	}

	/// Try to unwrap a Rust `char` from a Java `int`. See [`char_from_java_int`] for details.
	///
	/// # Errors
	///
	/// This method can fail with two kinds of errors:
	///
	/// * [`Error::WrongJValueType`]: `self` does not contain a Java `int`.
	/// * [`Error::InvalidUtf32`]: `self` contains a Java `int`, but it is not a valid UTF-32 unit.
	pub fn i_char(self) -> Result<char> {
		let char = self.i()?;

		char_from_java_int(char).map_err(|source| Error::InvalidUtf32 { char, source })
	}

	/// Try to unwrap to a long.
	pub const fn j(self) -> Result<jlong> {
		match self {
			Self::Long(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jlong", self.type_name())),
		}
	}

	/// Try to unwrap to a short.
	pub const fn s(self) -> Result<jshort> {
		match self {
			Self::Short(b) => Ok(b),
			_ => Err(Error::WrongJValueType("jshort", self.type_name())),
		}
	}

	/// Try to unwrap to a void.
	pub const fn v(self) -> Result<()> {
		match self {
			Self::Void => Ok(()),
			_ => Err(Error::WrongJValueType("void", self.type_name())),
		}
	}

	/// Converts a Rust `char` to a Java `int`. See [`char_to_java_int`] for details.
	#[must_use]
	pub fn int_from_char(char: char) -> Self {
		Self::Int(char_to_java_int(char))
	}
}
