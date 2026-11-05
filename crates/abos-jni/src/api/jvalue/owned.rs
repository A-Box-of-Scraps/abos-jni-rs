use super::*;

impl<'local> JValueOwned<'local> {
	/// Convert the enum to its jni-compatible equivalent.
	pub fn as_jni(&self) -> jvalue {
		self.borrow().as_jni()
	}

	/// Get the type name for the enum variant.
	pub fn type_name(&self) -> &'static str {
		self.borrow().type_name()
	}

	/// Get the primitive type for the enum variant. If it's not a primitive
	/// (i.e. an Object), returns None.
	pub fn primitive_type(&self) -> Option<Primitive> {
		self.borrow().primitive_type()
	}

	/// Check if the value is a `null` object reference.
	pub fn is_null(&self) -> bool {
		matches!(self, Self::Object(obj) if obj.as_raw().is_null())
	}

	/// Check if the value is a `null` object reference, and return a [Error::NullPtr] error if so,
	/// otherwise, return the value unchanged.
	///
	/// This can be useful with APIs where a null represents an error but the implementation doesn't
	/// throw a Java exception.
	///
	/// **Beware:** Since `null` object references may be a valid (and safe) in many contexts, you
	/// should only use this method if you are sure that `null` represents an error in the specific
	/// context where you're using it.
	///
	/// **Note**: In many cases if `null` represents an error then the API would throw a Java
	/// exception, which would be automatically translated to an [Error::JavaException] error by
	/// this crate.
	pub fn check_null(self) -> Result<Self> {
		match self {
			Self::Object(obj) => {
				if obj.as_raw().is_null() {
					Err(Error::NullPtr("Null Object"))
				} else {
					Ok(Self::Object(obj))
				}
			}
			_ => Ok(self),
		}
	}

	/// Try to unwrap to an Object.
	///
	/// See [`Self::into_object`] as a more-ergonomic alias.
	pub fn l(self) -> Result<JObject<'local>> {
		match self {
			Self::Object(obj) => Ok(obj),
			_ => Err(Error::WrongJValueType("object", self.type_name())),
		}
	}

	/// Try to unwrap to an Object.
	#[doc(alias = "l")]
	pub fn into_object(self) -> Result<JObject<'local>> {
		self.l()
	}

	/// Try to unwrap to a boolean.
	///
	/// See [`Self::into_bool`] as a more-ergonomic alias.
	pub fn z(self) -> Result<bool> {
		self.borrow().z()
	}

	/// Try to unwrap to a boolean.
	#[doc(alias = "z")]
	pub fn into_bool(self) -> Result<bool> {
		self.borrow().z()
	}

	/// Try to unwrap to a byte.
	///
	/// See [`Self::into_byte`] as a more-ergonomic alias.
	pub fn b(self) -> Result<jbyte> {
		self.borrow().b()
	}

	/// Try to unwrap to a byte.
	#[doc(alias = "b")]
	pub fn into_byte(self) -> Result<jbyte> {
		self.borrow().b()
	}

	/// Try to unwrap to a `jchar`.
	///
	/// See [`Self::into_char`] as a more-ergonomic alias.
	pub fn c(self) -> Result<jchar> {
		self.borrow().c()
	}

	/// Try to unwrap to a `jchar`.
	///
	/// **Warning:** A Java `char` is a UTF-16 code unit, which cannot always be converted to a Rust
	/// `char` (which is a Unicode scalar value).
	///
	/// See [`char_from_java`] for more information.
	#[doc(alias = "c")]
	pub fn into_char(self) -> Result<jchar> {
		self.borrow().c()
	}

	/// Try to unwrap a Java `char` and then convert it to a Rust `char`.
	///
	/// **Warning:** This conversion is likely to fail. Using it is not recommended. Prefer
	/// [`JValueOwned::i_char`] where possible. See [`char_from_java`] for more information.
	///
	/// # Errors
	///
	/// This method can fail with two kinds of errors:
	///
	/// * [`Error::WrongJValueType`]: `self` does not contain a Java `char`.
	/// * [`Error::InvalidUtf16`]: `self` contains a Java `char`, but it is one half of a surrogate
	///   pair.
	#[deprecated = "Use `into_char` to get the raw Java jchar, and then (attempt to) convert it to a Rust char using `char_from_java` if needed. This method is likely to fail and is not recommended."]
	pub fn c_char(self) -> Result<char> {
		let char = self.c()?;

		char_from_java(char).map_err(|source| Error::InvalidUtf16 { source })
	}

	/// Try to unwrap to a double.
	///
	/// See [`Self::into_double`] as a more-ergonomic alias.
	pub fn d(self) -> Result<jdouble> {
		self.borrow().d()
	}

	/// Try to unwrap to a double.
	#[doc(alias = "d")]
	pub fn into_double(self) -> Result<jdouble> {
		self.borrow().d()
	}

	/// Try to unwrap to a float.
	///
	/// See [`Self::into_float`] as a more-ergonomic alias.
	pub fn f(self) -> Result<jfloat> {
		self.borrow().f()
	}

	/// Try to unwrap to a float.
	#[doc(alias = "f")]
	pub fn into_float(self) -> Result<jfloat> {
		self.borrow().f()
	}

	/// Try to unwrap to an int.
	///
	/// See [`Self::into_int`] as a more-ergonomic alias.
	pub fn i(self) -> Result<jint> {
		self.borrow().i()
	}

	/// Try to unwrap to an int.
	#[doc(alias = "i")]
	pub fn into_int(self) -> Result<jint> {
		self.borrow().i()
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
		self.i_into_char()
	}

	/// Try to unwrap a Rust `char` from a Java `int`. See [`char_from_java_int`] for details.
	///
	/// # Errors
	///
	/// This method can fail with two kinds of errors:
	///
	/// * [`Error::WrongJValueType`]: `self` does not contain a Java `int`.
	/// * [`Error::InvalidUtf32`]: `self` contains a Java `int`, but it is not a valid UTF-32 unit.
	#[doc(alias = "i_char")]
	pub fn i_into_char(self) -> Result<char> {
		let char = self.i()?;

		char_from_java_int(char).map_err(|source| Error::InvalidUtf32 { char, source })
	}

	/// Try to unwrap to a long.
	///
	/// See [`Self::into_long`] as a more-ergonomic alias.
	pub fn j(self) -> Result<jlong> {
		self.borrow().j()
	}

	/// Try to unwrap to a long.
	#[doc(alias = "j")]
	pub fn into_long(self) -> Result<jlong> {
		self.borrow().j()
	}

	/// Try to unwrap to a short.
	///
	/// See [`Self::into_short`] as a more-ergonomic alias.
	pub fn s(self) -> Result<jshort> {
		self.borrow().s()
	}

	/// Try to unwrap to a short.
	#[doc(alias = "s")]
	pub fn into_short(self) -> Result<jshort> {
		self.borrow().s()
	}

	/// Try to unwrap to a void.
	///
	/// See [`Self::into_void`] as a more-ergonomic alias.
	pub fn v(self) -> Result<()> {
		self.borrow().v()
	}

	/// Try to unwrap to a void.
	#[doc(alias = "v")]
	pub fn into_void(self) -> Result<()> {
		self.borrow().v()
	}

	/// Copies or borrows the value in this `JValueOwned`.
	///
	/// If the value is a primitive type, it is copied. If the value is an
	/// object reference, it is borrowed.
	#[inline]
	pub fn borrow(&'_ self) -> JValue<'_> {
		match self {
			Self::Object(o) => JValue::Object(o),
			Self::Byte(v) => JValue::Byte(*v),
			Self::Char(v) => JValue::Char(*v),
			Self::Short(v) => JValue::Short(*v),
			Self::Int(v) => JValue::Int(*v),
			Self::Long(v) => JValue::Long(*v),
			Self::Bool(v) => JValue::Bool(*v),
			Self::Float(v) => JValue::Float(*v),
			Self::Double(v) => JValue::Double(*v),
			Self::Void => JValue::Void,
		}
	}
}
