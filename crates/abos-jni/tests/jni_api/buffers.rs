use super::*;

#[test]
pub fn get_object_class() {
	attach_current_thread(|env| {
		let string: abos_jni::objects::JString<'_> = env.new_string("test").expect("test string can be allocated");
		let result: std::result::Result<abos_jni::objects::JClass<'_>, abos_jni::errors::Error> =
			env.get_object_class(string);
		assert!(result.is_ok());
		assert!(!result.expect("test operation succeeds").is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_object_class_null_arg() {
	attach_current_thread(|env| {
		let null_obj: JObject<'_> = JObject::null();
		let result: bool = env
			.get_object_class(null_obj)
			.map_err(|error| matches!(error, Error::NullPtr(_)))
			.expect_err("Env#get_object_class should return error for null argument");
		assert!(result, "ErrorKind::NullPtr expected as error");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn new_direct_byte_buffer() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf: &mut [u8] = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		// SAFETY: The leaked allocation keeps this address valid for every Java reference to the buffer.
		let result: std::result::Result<JByteBuffer<'_>, abos_jni::errors::Error> =
			unsafe { env.new_direct_byte_buffer(addr, len) };
		assert!(result.is_ok());
		assert!(!result.expect("test operation succeeds").is_null());
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn new_direct_byte_buffer_invalid_addr() {
	attach_current_thread(|env| {
		// SAFETY: The null argument is rejected before any JNI call or memory access.
		let result: std::result::Result<JByteBuffer<'_>, abos_jni::errors::Error> =
			unsafe { env.new_direct_byte_buffer(std::ptr::null_mut(), 5) };
		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_capacity_ok() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf: &mut [u8] = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		// SAFETY: The leaked allocation keeps this address valid for every Java reference to the buffer.
		let result: JByteBuffer<'_> =
			unsafe { env.new_direct_byte_buffer(addr, len) }.expect("new direct byte buffer succeeds");
		assert!(!result.is_null());

		let capacity: usize = env
			.get_direct_buffer_capacity(&result)
			.expect("get direct buffer capacity succeeds");
		assert_eq!(capacity, 4);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_capacity_non_direct_buffer() {
	attach_current_thread(|env| {
		let buffer: JObject<'_> = env
			.call_static_method(
				jni_str!("java/nio/ByteBuffer"),
				jni_str!("allocate"),
				jni_sig!("(I)Ljava/nio/ByteBuffer;"),
				&[JValue::Int(4)],
			)?
			.l()?;
		let buffer: JByteBuffer<'_> = env.cast_local::<JByteBuffer>(buffer)?;
		let capacity: std::result::Result<usize, abos_jni::errors::Error> = env.get_direct_buffer_capacity(&buffer);
		assert!(capacity.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_capacity_null_arg() {
	attach_current_thread(|env| {
		let result: std::result::Result<usize, abos_jni::errors::Error> =
			env.get_direct_buffer_capacity(&abos_jni::objects::JByteBuffer::default());
		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_address_ok() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf: &mut [u8] = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		// SAFETY: The leaked allocation keeps this address valid for every Java reference to the buffer.
		let result: JByteBuffer<'_> =
			unsafe { env.new_direct_byte_buffer(addr, len) }.expect("new direct byte buffer succeeds");
		assert!(!result.is_null());

		let dest_buffer: *mut u8 = env
			.get_direct_buffer_address(&result)
			.expect("get direct buffer address succeeds");
		assert_eq!(addr, dest_buffer);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_address_wrong_arg() {
	attach_current_thread(|env| {
		let wrong_obj: JObject = env.new_string("wrong").expect("test string can be allocated").into();

		// SAFETY: This is not a valid cast and not generally safe but `GetDirectBufferAddress` is
		// documented to return a null pointer in case the "given object is not a direct java.nio.Buffer".
		let wrong_obj: JByteBuffer<'_> = unsafe { JByteBuffer::from_raw(env, wrong_obj.into_raw()) };
		let result: std::result::Result<*mut u8, abos_jni::errors::Error> = env.get_direct_buffer_address(&wrong_obj);
		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_direct_buffer_address_null_arg() {
	attach_current_thread(|env| {
		let result: std::result::Result<*mut u8, abos_jni::errors::Error> =
			env.get_direct_buffer_address(&JByteBuffer::null());
		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

// Group test for testing the family of new_PRIMITIVE_array functions with correct arguments
