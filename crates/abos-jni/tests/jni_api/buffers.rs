use super::*;

#[test]
pub fn get_object_class() {
	attach_current_thread(|env| {
		let string = env.new_string("test").unwrap();
		let result = env.get_object_class(string);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_object_class_null_arg() {
	attach_current_thread(|env| {
		let null_obj = JObject::null();
		let result = env
			.get_object_class(null_obj)
			.map_err(|error| matches!(error, Error::NullPtr(_)))
			.expect_err("Env#get_object_class should return error for null argument");
		assert!(result, "ErrorKind::NullPtr expected as error");

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn new_direct_byte_buffer() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		let result = unsafe { env.new_direct_byte_buffer(addr, len) };
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());
		Ok(())
	})
	.unwrap();
}

#[test]
pub fn new_direct_byte_buffer_invalid_addr() {
	attach_current_thread(|env| {
		let result = unsafe { env.new_direct_byte_buffer(std::ptr::null_mut(), 5) };
		assert!(result.is_err());

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_capacity_ok() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		let result = unsafe { env.new_direct_byte_buffer(addr, len) }.unwrap();
		assert!(!result.is_null());

		let capacity = env.get_direct_buffer_capacity(&result).unwrap();
		assert_eq!(capacity, 4);

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_capacity_wrong_arg() {
	attach_current_thread(|env| {
		let wrong_type = env.new_string("wrong").unwrap();
		let wrong_obj = unsafe { JByteBuffer::from_raw(env, wrong_type.into_raw()) };
		let capacity = env.get_direct_buffer_capacity(&wrong_obj);
		assert!(capacity.is_err());

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_capacity_null_arg() {
	attach_current_thread(|env| {
		let result = env.get_direct_buffer_capacity(&Default::default());
		assert!(result.is_err());

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_address_ok() {
	attach_current_thread(|env| {
		let vec: Vec<u8> = vec![0, 1, 2, 3];
		let (addr, len) = {
			// (would use buf.into_raw_parts() on nightly)
			let buf = vec.leak();
			(buf.as_mut_ptr(), buf.len())
		};
		let result = unsafe { env.new_direct_byte_buffer(addr, len) }.unwrap();
		assert!(!result.is_null());

		let dest_buffer = env.get_direct_buffer_address(&result).unwrap();
		assert_eq!(addr, dest_buffer);

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_address_wrong_arg() {
	attach_current_thread(|env| {
		let wrong_obj: JObject = env.new_string("wrong").unwrap().into();

		// SAFETY: This is not a valid cast and not generally safe but `GetDirectBufferAddress` is
		// documented to return a null pointer in case the "given object is not a direct java.nio.Buffer".
		let wrong_obj = unsafe { JByteBuffer::from_raw(env, wrong_obj.into_raw()) };
		let result = env.get_direct_buffer_address(&wrong_obj);
		assert!(result.is_err());

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_direct_buffer_address_null_arg() {
	attach_current_thread(|env| {
		let result = env.get_direct_buffer_address(&JByteBuffer::null());
		assert!(result.is_err());

		Ok(())
	})
	.unwrap();
}

// Group test for testing the family of new_PRIMITIVE_array functions with correct arguments
