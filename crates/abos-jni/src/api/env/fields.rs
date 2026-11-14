use super::{
	Desc, Env, Error, FieldSignature, IntoAuto, JClass, JFieldID, JNIStr, JObject, JStaticFieldID, JValue, JValueOwned,
	JavaType, Reference, Result,
};

impl<'local> Env<'local> {
	/// Get a field without checking the provided type against the actual field.
	///
	/// # Safety
	///
	/// - The `obj` must not be `null`
	/// - The `field` must be associated with the given `obj` (got from passing the `obj` to [`Env::get_field_id`])
	/// - The field must have the specified `ty` type.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub unsafe fn get_field_unchecked<'other_local, O, F>(
		&mut self,
		obj: O,
		field: F,
		ty: JavaType,
	) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		F: Desc<'local, JFieldID>,
	{
		use crate::signature::Primitive::{Boolean, Byte, Char, Double, Float, Int, Long, Short, Void};
		use JavaType::{Array, Object, Primitive};

		self.assert_top();

		let obj: &JObject<'_> = obj.as_ref();
		let obj: &JObject<'_> = null_check!(obj, "get_field_typed obj argument")?;

		let field: crate::sys::jfieldID = field.lookup(self)?.as_ref().into_raw();
		let obj: crate::sys::jobject = obj.as_raw();

		macro_rules! field {
			($get_field:ident) => {{
				// Safety: No exceptions are defined for Get*Field and we assume
				// the caller knows that the field is valid
				unsafe { JValueOwned::from(jni_call_no_post_check_ex!(self, v1_1, $get_field, obj, field)?) }
			}};
		}

		match ty {
			Object | Array => {
				// SAFETY: The caller guarantees the field ID and object match the supplied reference return type.
				let obj: JObject<'_> = unsafe {
					jni_call_post_check_ex!(self, v1_1, GetObjectField, obj, field)
						.map(|obj| JObject::from_raw(self, obj))?
				};
				Ok(obj.into())
			}
			Primitive(Char) => Ok(field!(GetCharField)),
			Primitive(Boolean) => Ok(field!(GetBooleanField)),
			Primitive(Short) => Ok(field!(GetShortField)),
			Primitive(Int) => Ok(field!(GetIntField)),
			Primitive(Long) => Ok(field!(GetLongField)),
			Primitive(Float) => Ok(field!(GetFloatField)),
			Primitive(Double) => Ok(field!(GetDoubleField)),
			Primitive(Byte) => Ok(field!(GetByteField)),
			Primitive(Void) => Err(Error::WrongJValueType("void", "see java field")),
		}
	}

	/// Set a field without any type checking.
	///
	/// # Safety
	///
	/// - The `obj` must not be `null`
	/// - The `field` must be associated with the given `obj` (got from passing the `obj` to [`Env::get_field_id`])
	/// - The field type must match the given `value` type.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub unsafe fn set_field_unchecked<'other_local, O, F>(&mut self, obj: O, field: F, value: JValue) -> Result<()>
	where
		O: AsRef<JObject<'other_local>>,
		F: Desc<'local, JFieldID>,
	{
		if matches!(value, JValue::Void) {
			return Err(Error::WrongJValueType("void", "see java field"));
		}

		let obj: &JObject<'_> = obj.as_ref();

		let field: crate::sys::jfieldID = field.lookup(self)?.as_ref().into_raw();
		let obj: crate::sys::jobject = obj.as_raw();

		macro_rules! set_field {
			// SAFETY: The caller guarantees that the object, field ID and value have matching JNI types.
			($set_field:ident($val:expr)) => {{ unsafe { jni_call_no_post_check_ex!(self, v1_1, $set_field, obj, field, $val) } }};
		}

		match value {
			JValue::Object(o) => set_field!(SetObjectField(o.as_raw()))?,
			JValue::Bool(b) => set_field!(SetBooleanField(b))?,
			JValue::Char(c) => set_field!(SetCharField(c))?,
			JValue::Short(s) => set_field!(SetShortField(s))?,
			JValue::Int(i) => set_field!(SetIntField(i))?,
			JValue::Long(l) => set_field!(SetLongField(l))?,
			JValue::Float(f) => set_field!(SetFloatField(f))?,
			JValue::Double(d) => set_field!(SetDoubleField(d))?,
			JValue::Byte(b) => set_field!(SetByteField(b))?,
			JValue::Void => (),
		}

		Ok(())
	}

	/// Get a field. Requires an object class lookup and a field id lookup
	/// internally.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub fn get_field<'other_local, 'sig, O, N, S>(&mut self, obj: O, name: N, sig: S) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<FieldSignature<'sig>>,
	{
		self.assert_top();
		let obj: &JObject<'_> = obj.as_ref();
		let obj: &JObject<'_> = null_check!(obj, "get_field obj argument")?;
		let class: crate::refs::Auto<'_, JClass<'_>> = self.get_object_class(obj)?.auto();

		let sig: &FieldSignature<'_> = sig.as_ref();
		let field_ty: JavaType = sig.ty();
		let field_id: JFieldID = Desc::<JFieldID>::lookup((&class, name, sig), self)?;

		// Safety: Since we have explicitly looked up the field ID based on the given
		// return type we have already validate that they match
		unsafe { self.get_field_unchecked(obj, field_id, field_ty) }
	}

	/// Set a field. Does the same lookups as `get_field` and ensures that the
	/// type matches the given value.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if a reference value is not assignable to the resolved field type.
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub fn set_field<'other_local, 'sig, O, N, S>(&mut self, obj: O, name: N, sig: S, value: JValue) -> Result<()>
	where
		O: AsRef<JObject<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<FieldSignature<'sig>>,
	{
		let obj: &JObject<'_> = obj.as_ref();
		let obj: &JObject<'_> = null_check!(obj, "set_field obj argument")?;
		let sig: &FieldSignature<'_> = sig.as_ref();

		if !sig.type_is_compatible_with(value.java_type()) {
			return Err(Error::WrongJValueType(value.type_name(), "see java field"));
		}

		let class: crate::refs::Auto<'_, JClass<'_>> = self.get_object_class(obj)?.auto();

		let field_id: JFieldID = Desc::<JFieldID>::lookup((&class, name, sig), self)?;
		if let JValue::Object(object) = value {
			// SAFETY: The field ID was resolved from this class and reference signature.
			unsafe {
				self.__validate_field_reference(&class, field_id.into_raw(), false, object)?;
			}
		}

		// SAFETY: The resolved field and value have matching primitive or reference types.
		unsafe { self.set_field_unchecked(obj, field_id, value) }
	}

	/// Get a static field without checking the provided type against the actual
	/// field.
	///
	/// # Safety
	///
	/// - The `class` must not be null
	/// - The `field` must be associated with the given `class` (got from passing the `class` to [`Env::get_static_field_id`])
	/// - The field must have the specified `ty` type.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub unsafe fn get_static_field_unchecked<'other_local, C, F>(
		&mut self,
		class: C,
		field: F,
		ty: JavaType,
	) -> Result<JValueOwned<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		F: Desc<'local, JStaticFieldID>,
	{
		use crate::signature::Primitive::{Boolean, Byte, Char, Double, Float, Int, Long, Short, Void};
		use JavaType::{Array, Object, Primitive};

		self.assert_top();

		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let field: <F as Desc<'_, JStaticFieldID>>::Output = field.lookup(self)?;

		macro_rules! field {
			($get_field:ident) => {{
				// SAFETY: The caller guarantees the static field ID and return type match the resolved class.
				unsafe {
					jni_call_post_check_ex!(
						self,
						v1_1,
						$get_field,
						class.as_ref().as_raw(),
						field.as_ref().into_raw()
					)?
				}
			}};
		}

		let ret: std::result::Result<JValueOwned<'_>, crate::errors::Error> = match ty {
			Primitive(Void) => Err(Error::WrongJValueType("void", "see java field")),
			Object | Array => {
				let obj: crate::sys::jobject = field!(GetStaticObjectField);
				// SAFETY: The JNI field access returned an object reference in the current local frame.
				let obj: JObject<'_> = unsafe { JObject::from_raw(self, obj) };
				Ok(JValueOwned::from(obj))
			}
			Primitive(Boolean) => Ok(field!(GetStaticBooleanField).into()),
			Primitive(Char) => Ok(field!(GetStaticCharField).into()),
			Primitive(Short) => Ok(field!(GetStaticShortField).into()),
			Primitive(Int) => Ok(field!(GetStaticIntField).into()),
			Primitive(Long) => Ok(field!(GetStaticLongField).into()),
			Primitive(Float) => Ok(field!(GetStaticFloatField).into()),
			Primitive(Double) => Ok(field!(GetStaticDoubleField).into()),
			Primitive(Byte) => Ok(field!(GetStaticByteField).into()),
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		ret
	}

	/// Set a static field. Requires a class lookup and a field id lookup internally.
	///
	/// # Safety
	///
	/// - The `class` must not be null
	/// - The `field` must be associated with the given `class` (got from passing the `class` to [`Env::get_static_field_id`])
	/// - The field type must match the given `value` type.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub unsafe fn set_static_field_unchecked<'other_local, C, F>(
		&mut self,
		class: C,
		field: F,
		value: JValue,
	) -> Result<()>
	where
		C: Desc<'local, JClass<'other_local>>,
		F: Desc<'local, JStaticFieldID>,
	{
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let field: <F as Desc<'_, JStaticFieldID>>::Output = field.lookup(self)?;

		macro_rules! set_field {
			($set_field:ident($val:expr)) => {{
				// SAFETY: The caller guarantees that the static field ID and value match the resolved class.
				unsafe {
					jni_call_no_post_check_ex!(
						self,
						v1_1,
						$set_field,
						class.as_ref().as_raw(),
						field.as_ref().into_raw(),
						$val
					)
				}
			}};
		}

		match value {
			JValue::Object(v) => set_field!(SetStaticObjectField(v.as_raw()))?,
			JValue::Byte(v) => set_field!(SetStaticByteField(v))?,
			JValue::Char(v) => set_field!(SetStaticCharField(v))?,
			JValue::Short(v) => set_field!(SetStaticShortField(v))?,
			JValue::Int(v) => set_field!(SetStaticIntField(v))?,
			JValue::Long(v) => set_field!(SetStaticLongField(v))?,
			JValue::Bool(v) => set_field!(SetStaticBooleanField(v))?,
			JValue::Float(v) => set_field!(SetStaticFloatField(v))?,
			JValue::Double(v) => set_field!(SetStaticDoubleField(v))?,
			JValue::Void => (),
		}

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		Ok(())
	}

	/// Get a static field. Requires a class lookup and a field id lookup
	/// internally.
	///
	/// # Errors
	///
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub fn get_static_field<'other_local, 'sig, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
	) -> Result<JValueOwned<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<FieldSignature<'sig>>,
	{
		self.assert_top();
		let sig: &FieldSignature<'_> = sig.as_ref();
		let field_ty: JavaType = sig.ty();

		// go ahead and look up the class since we'll need that for the next call.
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;

		// SAFETY: We have verified that `class`, `field`, `sig` and `ty` are valid
		unsafe { self.get_static_field_unchecked(class.as_ref(), (class.as_ref(), name, sig), field_ty) }
	}

	/// Set a static field. Requires a class lookup and a field id lookup internally.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if a reference value is not assignable to the resolved field type.
	/// Returns an error if an object or class is null, a member lookup fails, or a JNI field operation reports an exception.
	pub fn set_static_field<'other_local, 'sig, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
		value: JValue,
	) -> Result<()>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<FieldSignature<'sig>>,
	{
		let sig: &FieldSignature<'_> = sig.as_ref();

		if !sig.type_is_compatible_with(value.java_type()) {
			return Err(Error::WrongJValueType(value.type_name(), "see java field"));
		}

		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;

		let field_id: JStaticFieldID = *Desc::<JStaticFieldID>::lookup((class.as_ref(), name, sig), self)?.as_ref();
		if let JValue::Object(object) = value {
			// SAFETY: The field ID was resolved from this class and reference signature.
			unsafe {
				self.__validate_field_reference(class.as_ref(), field_id.into_raw(), true, object)?;
			}
		}

		// SAFETY: The resolved field and value have matching primitive or reference types.
		unsafe { self.set_static_field_unchecked(class.as_ref(), field_id, value) }
	}
}
