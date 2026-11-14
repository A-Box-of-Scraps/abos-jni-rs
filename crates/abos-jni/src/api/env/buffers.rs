use super::{Env, Error, JByteBuffer, JniError, Reference, Result, c_void, jlong};

impl<'local> Env<'local> {
	/// Create a new instance of a direct java.nio.ByteBuffer
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env};
	/// #
	/// # fn example(env: &mut Env) -> Result<()> {
	/// let buf = vec![0; 1024 * 1024];
	/// let (addr, len) = { // (use buf.into_raw_parts() on nightly)
	///     let buf = buf.leak();
	///     (buf.as_mut_ptr(), buf.len())
	/// };
	/// let direct_buffer = unsafe { env.new_direct_byte_buffer(addr, len) }?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Safety
	///
	/// Expects a valid (non-null) pointer and length
	///
	/// Caller must ensure the lifetime of `data` extends to all uses of the returned
	/// `ByteBuffer`. The JVM may maintain references to the `ByteBuffer` beyond the lifetime
	/// of this `Env`.
	///
	/// # Errors
	///
	/// Returns an error if the data pointer is null, the length is too large, or JNI cannot create the direct buffer.
	pub unsafe fn new_direct_byte_buffer(&mut self, data: *mut u8, len: usize) -> Result<JByteBuffer<'local>> {
		self.assert_top();
		let data: *mut u8 = null_check!(data, "new_direct_byte_buffer data argument")?;
		let len: i64 = jlong::try_from(len).map_err(|_| Error::JniCall(JniError::InvalidArguments))?;
		// Safety: jni-rs requires JNI >= 1.4
		unsafe {
			let obj: crate::sys::jobject = jni_call_with_catch_and_null_check!(
				catch |env| {
					crate::exceptions::JIllegalArgumentException =>
						Err(Error::JniCall(JniError::InvalidArguments)),
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::NullPtr("Unexpected Exception")),
				},
				self,
				v1_4,
				NewDirectByteBuffer,
				data.cast::<c_void>(),
				len
			)?;
			Ok(JByteBuffer::from_raw(self, obj))
		}
	}

	/// Returns the starting address of the memory of the direct
	/// java.nio.ByteBuffer.
	///
	/// # Safety
	///
	/// The caller must ensure the lifetime of `buf` extends to all uses of the
	/// returned pointer.
	///
	/// # Errors
	///
	/// Returns an error if the buffer is null, JNI cannot expose its direct storage, or a Java exception is pending.
	pub fn get_direct_buffer_address(&self, buf: &JByteBuffer) -> Result<*mut u8> {
		let buf: &JByteBuffer<'_> = null_check!(buf, "get_direct_buffer_address argument")?;
		// Safety: jni-rs requires JNI >= 1.4 and this is checked in `from_raw`
		unsafe {
			// GetDirectBufferAddress has no documented exceptions that it can throw
			let ptr: *mut std::ffi::c_void =
				jni_call_only_check_null_ret!(self, v1_4, GetDirectBufferAddress, buf.as_raw())?;
			Ok(ptr.cast())
		}
	}

	/// Returns the capacity (length) of the direct java.nio.ByteBuffer.
	///
	/// # Terminology
	///
	/// "capacity" here means the length that was passed to [`Self::new_direct_byte_buffer()`]
	/// which does not reflect the (potentially) larger size of the underlying allocation (unlike the `Vec`
	/// API).
	///
	/// The terminology is simply kept from the original JNI API (`GetDirectBufferCapacity`).
	///
	/// # Errors
	///
	/// Returns an error if the buffer is null, is not a supported direct buffer, or a Java exception is pending.
	pub fn get_direct_buffer_capacity(&self, buf: &JByteBuffer) -> Result<usize> {
		let buf: &JByteBuffer<'_> = null_check!(buf, "get_direct_buffer_capacity argument")?;
		// Safety: jni-rs requires JNI >= 1.4 and this is checked in `from_raw`
		unsafe {
			let capacity: i64 = jni_call_no_post_check_ex!(self, v1_4, GetDirectBufferCapacity, buf.as_raw())?;
			usize::try_from(capacity).map_err(|_| Error::JniCall(JniError::Unknown))
		}
	}
}
