use super::{
	Env, JBooleanArray, JByteArray, JCharArray, JDoubleArray, JFloatArray, JIntArray, JLongArray, JShortArray,
	Reference, Result, TypeArraySealed, jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jsize,
};

impl Env<'_> {
	/// Copy elements of the java boolean array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JBooleanArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_boolean_array_region<'other_local>(
		&self,
		array: impl AsRef<JBooleanArray<'other_local>>,
		start: jsize,
		buf: &mut [jboolean],
	) -> Result<()> {
		unsafe { <jboolean as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java byte array from the `start` index to the `buf`
	/// slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JByteArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_byte_array_region<'other_local>(
		&self,
		array: impl AsRef<JByteArray<'other_local>>,
		start: jsize,
		buf: &mut [jbyte],
	) -> Result<()> {
		unsafe { <jbyte as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java char array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JCharArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_char_array_region<'other_local>(
		&self,
		array: impl AsRef<JCharArray<'other_local>>,
		start: jsize,
		buf: &mut [jchar],
	) -> Result<()> {
		unsafe { <jchar as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java short array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JShortArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_short_array_region<'other_local>(
		&self,
		array: impl AsRef<JShortArray<'other_local>>,
		start: jsize,
		buf: &mut [jshort],
	) -> Result<()> {
		unsafe { <jshort as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java int array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JIntArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_int_array_region<'other_local>(
		&self,
		array: impl AsRef<JIntArray<'other_local>>,
		start: jsize,
		buf: &mut [jint],
	) -> Result<()> {
		unsafe { <jint as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java long array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JLongArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_long_array_region<'other_local>(
		&self,
		array: impl AsRef<JLongArray<'other_local>>,
		start: jsize,
		buf: &mut [jlong],
	) -> Result<()> {
		unsafe { <jlong as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java float array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JFloatArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_float_array_region<'other_local>(
		&self,
		array: impl AsRef<JFloatArray<'other_local>>,
		start: jsize,
		buf: &mut [jfloat],
	) -> Result<()> {
		unsafe { <jfloat as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy elements of the java double array from the `start` index to the
	/// `buf` slice. The number of copied elements is equal to the `buf` length.
	///
	/// # Errors
	/// If `start` is negative _or_ `start + buf.len()` is greater than [`array.length`]
	/// then no elements are copied, an `ArrayIndexOutOfBoundsException` is thrown,
	/// and `Err` is returned.
	///
	/// [`array.length`]: struct.Env.html#method.get_array_length
	#[deprecated(
		since = "0.22.0",
		note = "use JDoubleArray::get_region instead; this method will be removed in a future version"
	)]
	pub fn get_double_array_region<'other_local>(
		&self,
		array: impl AsRef<JDoubleArray<'other_local>>,
		start: jsize,
		buf: &mut [jdouble],
	) -> Result<()> {
		unsafe { <jdouble as TypeArraySealed>::get_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java boolean array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JBooleanArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_boolean_array_region<'other_local>(
		&self,
		array: impl AsRef<JBooleanArray<'other_local>>,
		start: jsize,
		buf: &[jboolean],
	) -> Result<()> {
		unsafe { <jboolean as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java byte array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JByteArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_byte_array_region<'other_local>(
		&self,
		array: impl AsRef<JByteArray<'other_local>>,
		start: jsize,
		buf: &[jbyte],
	) -> Result<()> {
		unsafe { <jbyte as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java char array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JCharArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_char_array_region<'other_local>(
		&self,
		array: impl AsRef<JCharArray<'other_local>>,
		start: jsize,
		buf: &[jchar],
	) -> Result<()> {
		unsafe { <jchar as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java short array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JShortArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_short_array_region<'other_local>(
		&self,
		array: impl AsRef<JShortArray<'other_local>>,
		start: jsize,
		buf: &[jshort],
	) -> Result<()> {
		unsafe { <jshort as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java int array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JIntArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_int_array_region<'other_local>(
		&self,
		array: impl AsRef<JIntArray<'other_local>>,
		start: jsize,
		buf: &[jint],
	) -> Result<()> {
		unsafe { <jint as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java long array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JLongArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_long_array_region<'other_local>(
		&self,
		array: impl AsRef<JLongArray<'other_local>>,
		start: jsize,
		buf: &[jlong],
	) -> Result<()> {
		unsafe { <jlong as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java float array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JFloatArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_float_array_region<'other_local>(
		&self,
		array: impl AsRef<JFloatArray<'other_local>>,
		start: jsize,
		buf: &[jfloat],
	) -> Result<()> {
		unsafe { <jfloat as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}

	/// Copy the contents of the `buf` slice to the java double array at the
	/// `start` index.
	#[deprecated(
		since = "0.22.0",
		note = "use JDoubleArray::set_region instead; this method will be removed in a future version"
	)]
	pub fn set_double_array_region<'other_local>(
		&self,
		array: impl AsRef<JDoubleArray<'other_local>>,
		start: jsize,
		buf: &[jdouble],
	) -> Result<()> {
		unsafe { <jdouble as TypeArraySealed>::set_region(self, array.as_ref().as_raw(), start, buf) }
	}
}
