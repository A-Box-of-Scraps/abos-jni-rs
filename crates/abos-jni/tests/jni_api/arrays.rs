use super::*;

#[test]
pub fn java_byte_array_from_slice() {
	attach_current_thread(|env| {
		let buf: &[u8] = &[1, 2, 3];
		let java_array = env
			.byte_array_from_slice(buf)
			.expect("Env#byte_array_from_slice must create a java array from slice")
			.auto();

		assert!(!java_array.is_null());
		let mut res: [i8; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).unwrap();
		assert_eq!(res[0], 1);
		assert_eq!(res[1], 2);
		assert_eq!(res[2], 3);

		Ok(())
	})
	.unwrap();
}

macro_rules! test_auto_array_read_write {
    ( $test_name:tt, $jni_type:ty, $new_array:tt, $value_a:tt, $value_b:tt ) => {
        #[test]
        pub fn $test_name() {
            attach_current_thread(|env| {
                // Create original Java array
                let buf: &[$jni_type] = &[$value_a as $jni_type, $value_b as $jni_type];
                let java_array = env
                    .$new_array(2)
                    .expect(stringify!(Env #$new_array must create a Java $jni_type array with given size));

                // Insert array elements
                let _ = java_array.set_region(env, 0, buf);

                // Use a scope to test Drop
                {
                    // Redundantly push a new JNI stack frame to verify that AutoElements is not
                    // tied to the lifetime of the `env` that it's got from (env.get_array_elements()
                    // doesn't involve creating a new reference, it associates a pointer with
                    // an existing array reference)
                    let mut auto_ptr = env.with_local_frame(10, |env| -> abos_jni::errors::Result<_> {
                        // Get byte array elements auto wrapper
                        let auto_ptr: AutoElements<$jni_type, _> = unsafe {
                            java_array.get_elements(env, ReleaseMode::CopyBack).unwrap()
                        };
                        Ok(auto_ptr)
                    }).unwrap();

                    // Check array size
                    assert_eq!(auto_ptr.len(), 2);

                    // Check pointer access
                    let ptr = auto_ptr.as_ptr();
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    // Check pointer From access
                    let ptr: *mut $jni_type = std::convert::From::from(&auto_ptr);
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    // Check pointer into() access
                    let ptr: *mut $jni_type = (&auto_ptr).into();
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    // Check slice access
                    //
                    // # Safety
                    //
                    // We make sure that the slice is dropped before also testing access via `Deref`
                    // (to ensure we don't have aliased references)
                    unsafe {
                        let slice = std::slice::from_raw_parts(auto_ptr.as_ptr(), auto_ptr.len());
                        assert_eq!(slice[0], $value_a);
                        assert_eq!(slice[1], $value_b);
                    }

                    // Check access via Deref
                    assert_eq!(auto_ptr[0], $value_a);
                    assert_eq!(auto_ptr[1], $value_b);

                    // Modify via DerefMut
                    let tmp = auto_ptr[1];
                    auto_ptr[1] = auto_ptr[0];
                    auto_ptr[0] = tmp;

                    // Commit would be necessary here, if there were no closure
                    //auto_ptr.commit().unwrap();
                }

                // Confirm modification of original Java array
                let mut res: [$jni_type; 2] = [$value_a as $jni_type; 2];
                java_array.get_region(env, 0, &mut res).unwrap();
                assert_eq!(res[0], $value_b);
                assert_eq!(res[1], $value_a);
                Ok(())
            }).unwrap();
        }
    };
}

// Test generic get_array_elements
test_auto_array_read_write!(get_array_elements, jint, new_int_array, 0, 1);

// Test type-specific array accessors
test_auto_array_read_write!(get_int_array_elements, jint, new_int_array, 0, 1);

test_auto_array_read_write!(get_long_array_elements, jlong, new_long_array, 0, 1);

test_auto_array_read_write!(get_byte_array_elements, jbyte, new_byte_array, 0, 1);

test_auto_array_read_write!(get_boolean_array_elements, jboolean, new_boolean_array, true, false);

test_auto_array_read_write!(get_char_array_elements, jchar, new_char_array, 0, 1);

test_auto_array_read_write!(get_short_array_elements, jshort, new_short_array, 0, 1);

test_auto_array_read_write!(get_float_array_elements, jfloat, new_float_array, 0.0, 1.0);

test_auto_array_read_write!(get_double_array_elements, jdouble, new_double_array, 0.0, 1.0);

#[test]
#[ignore] // Disabled until issue #283 is resolved
pub fn get_long_array_elements_commit() {
	attach_current_thread(|env| {
		// Create original Java array
		let buf: &[i64] = &[1, 2, 3];
		let java_array = env
			.new_long_array(3)
			.expect("Env#new_long_array must create a java array with given size");

		// Insert array elements
		let _ = java_array.set_region(env, 0, buf);

		// Get long array elements auto wrapper
		let mut auto_ptr = unsafe { java_array.get_elements(env, ReleaseMode::CopyBack).unwrap() };

		// Copying the array depends on the VM vendor/version/GC combinations.
		// If the wrapped array is not being copied, we can skip the test.
		if !auto_ptr.is_copy() {
			return Ok(());
		}

		// Check pointer access
		let ptr = auto_ptr.as_ptr();

		// Modify
		unsafe {
			*ptr.offset(0) += 1;
			*ptr.offset(1) += 1;
			*ptr.offset(2) += 1;
		}

		// Check that original Java array is unmodified
		let mut res: [i64; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).unwrap();
		assert_eq!(res[0], 1);
		assert_eq!(res[1], 2);
		assert_eq!(res[2], 3);

		auto_ptr.commit().unwrap();

		// Confirm modification of original Java array
		java_array.get_region(env, 0, &mut res).unwrap();
		assert_eq!(res[0], 2);
		assert_eq!(res[1], 3);
		assert_eq!(res[2], 4);

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn get_array_elements_critical() {
	attach_current_thread(|env| {
		// Create original Java array
		let buf: &[u8] = &[1, 2, 3];
		let java_array = env
			.byte_array_from_slice(buf)
			.expect("Env#byte_array_from_slice must create a java array from slice");

		// Use a scope to test Drop
		{
			// Get primitive array elements auto wrapper
			let mut auto_ptr = unsafe { java_array.get_elements_critical(env, ReleaseMode::CopyBack).unwrap() };

			// Check array size
			assert_eq!(auto_ptr.len(), 3);

			// Convert void pointer to a &[i8] slice, without copy
			//
			// # Safety
			//
			// We make sure that the slice is dropped before also testing access via `Deref`
			// (to ensure we don't have aliased references)
			unsafe {
				let slice = std::slice::from_raw_parts(auto_ptr.as_ptr(), auto_ptr.len());
				assert_eq!(slice[0], 1);
				assert_eq!(slice[1], 2);
				assert_eq!(slice[2], 3);
			}

			// Also check access via `Deref`
			assert_eq!(auto_ptr[0], 1);
			assert_eq!(auto_ptr[1], 2);
			assert_eq!(auto_ptr[2], 3);

			// Modify via `DerefMut`
			auto_ptr[0] += 1;
			auto_ptr[1] += 1;
			auto_ptr[2] += 1;
		}

		// Confirm modification of original Java array
		let mut res: [i8; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).unwrap();
		assert_eq!(res[0], 2);
		assert_eq!(res[1], 3);
		assert_eq!(res[2], 4);

		Ok(())
	})
	.unwrap();
}

#[test]
pub fn new_primitive_array_ok() {
	attach_current_thread(|env| {
		const SIZE: usize = 16;

		let result = env.new_boolean_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_byte_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_char_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_short_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_int_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_long_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_float_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		let result = env.new_double_array(SIZE);
		assert!(result.is_ok());
		assert!(!result.unwrap().is_null());

		Ok(())
	})
	.unwrap();
}

// Group test for testing the family of new_PRIMITIVE_array functions with wrong arguments
#[test]
pub fn new_primitive_array_bad_size() {
	attach_current_thread(|env| {
		const BAD_SIZE: usize = usize::MAX;

		let result = env.new_boolean_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_byte_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_char_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_short_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_int_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_long_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_float_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result = env.new_double_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		Ok(())
	})
	.unwrap();
}

#[test]
fn convert_byte_array() {
	attach_current_thread(|env| {
		let src: Vec<u8> = vec![1, 2, 3, 4];
		let java_byte_array = env.byte_array_from_slice(&src).unwrap();

		let dest = env.convert_byte_array(java_byte_array);
		assert!(dest.is_ok());
		assert_eq!(dest.unwrap(), src);

		Ok(())
	})
	.unwrap();
}
