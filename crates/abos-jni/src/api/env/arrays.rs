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
	pub fn get_array_length<'other_local, 'array>(
		&self,
		array: &'array impl AsJArrayRaw<'other_local>,
	) -> Result<jsize> {
		let array = null_check!(array.as_jarray_raw(), "get_array_length array argument")?;
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
		let class = element_class.lookup(self)?;

		let array = unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_1,
				NewObjectArray,
				length,
				class.as_ref().as_raw(),
				initial_element.as_ref().as_raw()
			)
			.map(|array| JObjectArray::<JObject>::from_raw(self, array))?
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		Ok(array)
	}

	/// Construct a new array holding objects with of type `E`.
	///
	/// All elements are initially set to `initial_element`.
	///
	/// It's recommended to use [`JObjectArray::new`] instead.
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
	pub fn set_object_array_element<'any_local_1, 'any_local_2, E: Reference + 'any_local_1>(
		&self,
		array: impl AsRef<JObjectArray<'any_local_1, E>>,
		index: usize,
		value: impl AsRef<E::Kind<'any_local_2>>,
	) -> Result<()> {
		array.as_ref().set_element(self, index, value)
	}

	/// Create a new java byte array from a rust byte slice.
	pub fn byte_array_from_slice(&mut self, buf: &[u8]) -> Result<JByteArray<'local>> {
		let bytes = JByteArray::new(self, buf.len())?;
		// SAFETY: i8 and u8 are both plain, single-byte, data types with the same size and
		// alignment.
		//
		// We just want to forward the bitwise representation of the slice and it doesn't matter
		// whether we interpret the data as signed or unsigned.
		let buf: &[i8] = unsafe { std::mem::transmute(buf) };
		bytes.set_region(self, 0, buf)?;
		Ok(bytes)
	}

	/// Converts a java byte array to a rust vector of bytes.
	///
	/// Returns [`Error::IndexOutOfBounds`] if one of the calculated indexes in the region is not
	/// valid (in case the array length is changed concurrently).
	///
	/// This API catches exceptions internally and is not expected to return [`Error::JavaException`]
	pub fn convert_byte_array<'other_local>(&self, array: impl AsRef<JByteArray<'other_local>>) -> Result<Vec<u8>> {
		let array = array.as_ref().as_raw();
		let array = null_check!(array, "convert_byte_array array argument")?;

		// GetArrayLength is not documented to throw any exceptions
		let length = unsafe { jni_call_no_post_check_ex!(self, v1_1, GetArrayLength, array)? };
		let mut vec = vec![0u8; length as usize];
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
	pub fn new_boolean_array(&mut self, length: usize) -> Result<JBooleanArray<'local>> {
		JBooleanArray::new(self, length)
	}

	/// Create a new [`JByteArray`] of supplied length.
	///
	/// It is recommended to use [`JByteArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_byte_array(&mut self, length: usize) -> Result<JByteArray<'local>> {
		JByteArray::new(self, length)
	}

	/// Create a new [`JCharArray`] of supplied length.
	///
	/// It is recommended to use [`JCharArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_char_array(&mut self, length: usize) -> Result<JCharArray<'local>> {
		JCharArray::new(self, length)
	}

	/// Create a new [`JShortArray`] of supplied length.
	///
	/// It is recommended to use [`JShortArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_short_array(&mut self, length: usize) -> Result<JShortArray<'local>> {
		JShortArray::new(self, length)
	}

	/// Create a new [`JIntArray`] of supplied length.
	///
	/// It is recommended to use [`JIntArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_int_array(&mut self, length: usize) -> Result<JIntArray<'local>> {
		JIntArray::new(self, length)
	}

	/// Create a new [`JLongArray`] of supplied length.
	///
	/// It is recommended to use [`JLongArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_long_array(&mut self, length: usize) -> Result<JLongArray<'local>> {
		JLongArray::new(self, length)
	}

	/// Create a new [`JFloatArray`] of supplied length.
	///
	/// It is recommended to use [`JFloatArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_float_array(&mut self, length: usize) -> Result<JFloatArray<'local>> {
		JFloatArray::new(self, length)
	}

	/// Create a new [`JDoubleArray`] of supplied length.
	///
	/// It is recommended to use [`JDoubleArray::new`] instead.
	///
	/// This API may be deprecated and removed in a future version.
	pub fn new_double_array(&mut self, length: usize) -> Result<JDoubleArray<'local>> {
		JDoubleArray::new(self, length)
	}
}
