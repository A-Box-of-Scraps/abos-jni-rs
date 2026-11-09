#[cfg(doc)]
use crate::objects::JPrimitiveArray;

use super::{
	AsJArrayRaw, Desc, Env, Error, JBooleanArray, JByteArray, JCharArray, JClass, JDoubleArray, JFloatArray, JIntArray,
	JLongArray, JObject, JObjectArray, JShortArray, JniError, Reference, Result, jsize,
};

impl<'local> Env<'local> {
	/// Get the length of a [`JPrimitiveArray`] or [`JObjectArray`].
	#[deprecated(
		since = "0.22.0",
		note = "use JPrimitiveArray::len or JObjectArray::len instead. This method will be removed in a future version"
	)]
	///
	/// # Errors
	///
	/// Returns an error if the array is null or a Java exception is pending.
	pub fn get_array_length<'other_local, 'array>(
		&self,
		array: &'array impl AsJArrayRaw<'other_local>,
	) -> Result<jsize> {
		let array: crate::sys::jobject = null_check!(array.as_jarray_raw(), "get_array_length array argument")?;
		// SAFETY: AsJArrayRaw guarantees an array reference, which was checked for null.
		let len: jsize = unsafe { jni_call_no_post_check_ex!(self, v1_1, GetArrayLength, array)? };
		Ok(len)
	}

	/// Construct a new array holding objects in class `element_class`.
	///
	/// All elements are initially set to `initial_element`.
	///
	/// It's recommended to use [`JObjectArray::new`] or [`Self::new_object_type_array`]
	/// instead, which both support element type parameterization.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns [`Error::NullPtr`] if `element_class` is null.
	/// Returns [`Error::WrongObjectType`] if `element_class` represents a primitive type (including
	/// `void`), or if a non-null initializer is not assignable to it.
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_object_array<'other_local_1, 'other_local_2, T, U>(
		&mut self,
		length: jsize,
		element_class: T,
		initial_element: U,
	) -> Result<JObjectArray<'local>>
	where
		T: Desc<'local, JClass<'other_local_2>>,
		U: AsRef<JObject<'other_local_1>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class: <T as Desc<'_, JClass<'_>>>::Output = element_class.lookup(self)?;

		// Evaluate user-provided AsRef implementations once so validation and allocation
		// use the same class and initializer references.
		let element_class: &JClass<'_> = class.as_ref();
		let element_class: &JClass<'_> = null_check!(element_class, "new_object_array element_class argument")?;
		// NewObjectArray requires a reference component type, even with a null initializer.
		if element_class.is_primitive(self)? {
			return Err(Error::WrongObjectType);
		}
		let initial_element: &JObject<'_> = initial_element.as_ref();
		if !initial_element.is_null() && !self.is_instance_of_class(initial_element, element_class)? {
			return Err(Error::WrongObjectType);
		}

		// SAFETY: The class is non-null and non-primitive, the initializer is null or assignable to it,
		// and both references remain live during the JNI call.
		let array: JObjectArray<'_> = unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_1,
				NewObjectArray,
				length,
				element_class.as_raw(),
				initial_element.as_raw()
			)
			.map(|array| JObjectArray::<JObject>::from_raw(self, array))?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		Ok(array)
	}

	/// Construct a new array holding objects of type `E`.
	///
	/// All elements are initially set to `initial_element`.
	///
	/// It's recommended to use [`JObjectArray::new`] instead.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_object_type_array<'any_local, E>(
		&mut self,
		length: usize,
		initial_element: impl AsRef<E::Kind<'any_local>>,
	) -> Result<JObjectArray<'local, E::Kind<'local>>>
	where
		E: Reference,
	{
		JObjectArray::<E>::new(self, length, initial_element)
	}

	/// Returns a local reference to an element of the [`JObjectArray`] `array`.
	#[deprecated(
		since = "0.22.0",
		note = "use JObjectArray::get_element instead. This method will be removed in a future version"
	)]
	///
	/// # Errors
	///
	/// Returns an error if the array is null, the index is invalid, or a Java exception is pending.
	pub fn get_object_array_element<'other_local, E: Reference + 'other_local>(
		&mut self,
		array: impl AsRef<JObjectArray<'other_local, E>>,
		index: usize,
	) -> Result<E::Kind<'local>> {
		array.as_ref().get_element(self, index)
	}

	/// Sets an element of the [`JObjectArray`] `array`.
	#[deprecated(
		since = "0.22.0",
		note = "use JObjectArray::set_element instead. This method will be removed in a future version"
	)]
	///
	/// # Errors
	///
	/// Returns an error if the array is null, the index is invalid, the value has an incompatible type, or a Java exception is pending.
	pub fn set_object_array_element<'any_local_1, 'any_local_2, E: Reference + 'any_local_1>(
		&self,
		array: impl AsRef<JObjectArray<'any_local_1, E>>,
		index: usize,
		value: impl AsRef<E::Kind<'any_local_2>>,
	) -> Result<()> {
		array.as_ref().set_element(self, index, value)
	}

	/// Create a new java byte array from a rust byte slice.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn byte_array_from_slice(&mut self, buf: &[u8]) -> Result<JByteArray<'local>> {
		let bytes: JByteArray<'_> = JByteArray::new(self, buf.len())?;
		// SAFETY: i8 and u8 are both plain, single-byte, data types with the same size and
		// alignment.
		//
		// We just want to forward the bitwise representation of the slice and it doesn't matter
		// whether we interpret the data as signed or unsigned.
		let buf: &[i8] = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<i8>(), buf.len()) };
		bytes.set_region(self, 0, buf)?;
		Ok(bytes)
	}

	/// Copies a Java byte array into a Rust vector, preserving each byte's bit pattern.
	///
	/// Exceptions raised while copying are caught and mapped to Rust errors. An exception
	/// already pending on entry returns [`Error::JavaException`] without being cleared.
	///
	/// # Errors
	///
	/// Returns an error if the array is null, copying its contents fails, or a Java exception is pending.
	pub fn convert_byte_array<'other_local>(&self, array: impl AsRef<JByteArray<'other_local>>) -> Result<Vec<u8>> {
		let array: crate::sys::jobject = array.as_ref().as_raw();
		let array: crate::sys::jobject = null_check!(array, "convert_byte_array array argument")?;

		// GetArrayLength is not documented to throw any exceptions
		// SAFETY: The byte-array reference was checked for null and is valid for this environment.
		let length: i32 = unsafe { jni_call_no_post_check_ex!(self, v1_1, GetArrayLength, array)? };
		let buffer_len: usize = usize::try_from(length).map_err(|_| Error::JniCall(JniError::Unknown))?;
		let mut vec: Vec<u8> = vec![0u8; buffer_len];
		// SAFETY: The destination is allocated for the array length, and the byte-array reference remains live.
		unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JArrayIndexOutOfBoundsException =>
						Err(Error::IndexOutOfBounds),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				self,
				v1_1,
				GetByteArrayRegion,
				array,
				0,
				length,
				vec.as_mut_ptr().cast::<i8>()
			)?;
		}
		Ok(vec)
	}

	/// Create a new [`JBooleanArray`] of supplied length.
	///
	/// It is recommended to use [`JBooleanArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_boolean_array(&mut self, length: usize) -> Result<JBooleanArray<'local>> {
		JBooleanArray::new(self, length)
	}

	/// Create a new [`JByteArray`] of supplied length.
	///
	/// It is recommended to use [`JByteArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_byte_array(&mut self, length: usize) -> Result<JByteArray<'local>> {
		JByteArray::new(self, length)
	}

	/// Create a new [`JCharArray`] of supplied length.
	///
	/// It is recommended to use [`JCharArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_char_array(&mut self, length: usize) -> Result<JCharArray<'local>> {
		JCharArray::new(self, length)
	}

	/// Create a new [`JShortArray`] of supplied length.
	///
	/// It is recommended to use [`JShortArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_short_array(&mut self, length: usize) -> Result<JShortArray<'local>> {
		JShortArray::new(self, length)
	}

	/// Create a new [`JIntArray`] of supplied length.
	///
	/// It is recommended to use [`JIntArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_int_array(&mut self, length: usize) -> Result<JIntArray<'local>> {
		JIntArray::new(self, length)
	}

	/// Create a new [`JLongArray`] of supplied length.
	///
	/// It is recommended to use [`JLongArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_long_array(&mut self, length: usize) -> Result<JLongArray<'local>> {
		JLongArray::new(self, length)
	}

	/// Create a new [`JFloatArray`] of supplied length.
	///
	/// It is recommended to use [`JFloatArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_float_array(&mut self, length: usize) -> Result<JFloatArray<'local>> {
		JFloatArray::new(self, length)
	}

	/// Create a new [`JDoubleArray`] of supplied length.
	///
	/// It is recommended to use [`JDoubleArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	///
	/// # Errors
	///
	/// Returns an error if the array length is too large, class resolution or allocation fails, or a Java exception is pending.
	pub fn new_double_array(&mut self, length: usize) -> Result<JDoubleArray<'local>> {
		JDoubleArray::new(self, length)
	}
}
