use super::*;

#[test]
pub fn java_byte_array_from_slice() {
	attach_current_thread(|env| {
		let buf: &[u8] = &[1, 2, 3];
		let java_array: abos_jni::refs::Auto<'_, abos_jni::objects::JPrimitiveArray<'_, i8>> = env
			.byte_array_from_slice(buf)
			.expect("Env#byte_array_from_slice must create a java array from slice")
			.auto();

		assert!(!java_array.is_null());
		let mut res: [i8; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).expect("get region succeeds");
		assert_eq!(res[0], 1);
		assert_eq!(res[1], 2);
		assert_eq!(res[2], 3);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

macro_rules! test_auto_array_read_write {
    ( $test_name:tt, $jni_type:ty, $new_array:tt, $value_a:tt, $value_b:tt ) => {
        #[test]
        pub fn $test_name() {
            attach_current_thread(|env| {
                let buf: &[$jni_type] = &[$value_a as $jni_type, $value_b as $jni_type];
                let java_array = env
                    .$new_array(2)
                    .expect(stringify!(Env #$new_array must create a Java $jni_type array with given size));

                java_array.set_region(env, 0, buf)?;

                {
                    // A nested frame verifies that AutoElements borrows the array, not the Env used to acquire it.
                    let mut auto_ptr = env.with_local_frame(10, |env| -> abos_jni::errors::Result<_> {
                        // SAFETY: The test owns the array and does not allow concurrent access while its element guard is live.
                        let auto_ptr: AutoElements<$jni_type, _> = unsafe {
                            java_array.get_elements(env, ReleaseMode::CopyBack).unwrap()
                        };
                        Ok(auto_ptr)
                    }).unwrap();

                    assert_eq!(auto_ptr.len(), 2);

                    let ptr = auto_ptr.as_ptr();
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    let ptr: *mut $jni_type = std::convert::From::from(&auto_ptr);
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    let ptr: *mut $jni_type = (&auto_ptr).into();
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(0) }, $value_a);
                    // SAFETY: The live element guard provides both initialized array entries, and no mutable reference is active.
                    assert_eq!(unsafe { *ptr.offset(1) }, $value_b);

                    // SAFETY: The guard retains the initialized elements, and this shared slice is dropped before mutable access.
                    unsafe {
                        let slice = std::slice::from_raw_parts(auto_ptr.as_ptr(), auto_ptr.len());
                        assert_eq!(slice[0], $value_a);
                        assert_eq!(slice[1], $value_b);
                    }

                    assert_eq!(auto_ptr[0], $value_a);
                    assert_eq!(auto_ptr[1], $value_b);

                    let tmp = auto_ptr[1];
                    auto_ptr[1] = auto_ptr[0];
                    auto_ptr[0] = tmp;

                }

                let mut res: [$jni_type; 2] = [$value_a as $jni_type; 2];
                java_array.get_region(env, 0, &mut res).unwrap();
                assert_eq!(res[0], $value_b);
                assert_eq!(res[1], $value_a);
                Ok(())
            }).unwrap();
        }
    };
}

test_auto_array_read_write!(get_array_elements, jint, new_int_array, 0, 1);

test_auto_array_read_write!(get_int_array_elements, jint, new_int_array, 0, 1);

test_auto_array_read_write!(get_long_array_elements, jlong, new_long_array, 0, 1);

test_auto_array_read_write!(get_byte_array_elements, jbyte, new_byte_array, 0, 1);

test_auto_array_read_write!(get_boolean_array_elements, jboolean, new_boolean_array, true, false);

test_auto_array_read_write!(get_char_array_elements, jchar, new_char_array, 0, 1);

test_auto_array_read_write!(get_short_array_elements, jshort, new_short_array, 0, 1);

test_auto_array_read_write!(get_float_array_elements, jfloat, new_float_array, 0.0, 1.0);

test_auto_array_read_write!(get_double_array_elements, jdouble, new_double_array, 0.0, 1.0);

#[test]
#[ignore = "Disabled until issue #283 is resolved"]
pub fn get_long_array_elements_commit() {
	attach_current_thread(|env| {
		let buf: &[i64] = &[1, 2, 3];
		let java_array: abos_jni::objects::JPrimitiveArray<'_, i64> = env
			.new_long_array(3)
			.expect("Env#new_long_array must create a java array with given size");

		java_array.set_region(env, 0, buf)?;

		// SAFETY: The test owns the array and does not allow concurrent access while its element guard is live.
		let mut auto_ptr: AutoElements<'_, i64, &abos_jni::objects::JPrimitiveArray<'_, i64>> = unsafe {
			java_array
				.get_elements(env, ReleaseMode::CopyBack)
				.expect("get elements succeeds")
		};

		// Copying the array depends on the VM vendor/version/GC combinations.
		// If the wrapped array is not being copied, we can skip the test.
		if !auto_ptr.is_copy() {
			return Ok(());
		}

		let ptr: *mut i64 = auto_ptr.as_ptr();

		// SAFETY: The guard owns three writable elements; no slice or other element access overlaps these writes.
		unsafe {
			*ptr += 1;
			*ptr.add(1) += 1;
			*ptr.add(2) += 1;
		}

		let mut res: [i64; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).expect("get region succeeds");
		assert_eq!(res[0], 1);
		assert_eq!(res[1], 2);
		assert_eq!(res[2], 3);

		auto_ptr.commit().expect("commit succeeds");

		java_array.get_region(env, 0, &mut res).expect("get region succeeds");
		assert_eq!(res[0], 2);
		assert_eq!(res[1], 3);
		assert_eq!(res[2], 4);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn get_array_elements_critical() {
	attach_current_thread(|env| {
		let buf: &[u8] = &[1, 2, 3];
		let java_array: abos_jni::objects::JPrimitiveArray<'_, i8> = env
			.byte_array_from_slice(buf)
			.expect("Env#byte_array_from_slice must create a java array from slice");

		{
			// SAFETY: The test owns the array and does not allow concurrent access while its element guard is live.
			let mut auto_ptr: abos_jni::elements::AutoElementsCritical<
				'_,
				i8,
				&abos_jni::objects::JPrimitiveArray<'_, i8>,
			> = unsafe {
				java_array
					.get_elements_critical(env, ReleaseMode::CopyBack)
					.expect("get elements critical succeeds")
			};

			assert_eq!(auto_ptr.len(), 3);

			// SAFETY: The guard retains the initialized elements, and this shared slice is dropped before mutable access.
			unsafe {
				let slice: &[i8] = std::slice::from_raw_parts(auto_ptr.as_ptr(), auto_ptr.len());
				assert_eq!(slice[0], 1);
				assert_eq!(slice[1], 2);
				assert_eq!(slice[2], 3);
			}

			assert_eq!(auto_ptr[0], 1);
			assert_eq!(auto_ptr[1], 2);
			assert_eq!(auto_ptr[2], 3);

			auto_ptr[0] += 1;
			auto_ptr[1] += 1;
			auto_ptr[2] += 1;
		}

		let mut res: [i8; 3] = [0; 3];
		java_array.get_region(env, 0, &mut res).expect("get region succeeds");
		assert_eq!(res[0], 2);
		assert_eq!(res[1], 3);
		assert_eq!(res[2], 4);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn new_primitive_array_ok() {
	attach_current_thread(|env| {
		const SIZE: usize = 16;

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, bool>, abos_jni::errors::Error> =
			env.new_boolean_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, i8>, abos_jni::errors::Error> =
			env.new_byte_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, u16>, abos_jni::errors::Error> =
			env.new_char_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, i16>, abos_jni::errors::Error> =
			env.new_short_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, i32>, abos_jni::errors::Error> =
			env.new_int_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, i64>, abos_jni::errors::Error> =
			env.new_long_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, f32>, abos_jni::errors::Error> =
			env.new_float_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		let result: std::result::Result<abos_jni::objects::JPrimitiveArray<'_, f64>, abos_jni::errors::Error> =
			env.new_double_array(SIZE);
		assert!(!result.expect("test operation succeeds").is_null());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
pub fn new_primitive_array_bad_size() {
	attach_current_thread(|env| {
		const BAD_SIZE: usize = usize::MAX;

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_boolean_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_byte_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_char_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_short_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_int_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_long_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_float_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		let result: std::result::Result<abos_jni::sys::jobject, abos_jni::errors::Error> =
			env.new_double_array(BAD_SIZE).map(|arr| arr.as_raw());
		assert!(result.is_err());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn convert_byte_array() {
	attach_current_thread(|env| {
		let src: Vec<u8> = vec![1, 2, 3, 4];
		let java_byte_array: abos_jni::objects::JPrimitiveArray<'_, i8> =
			env.byte_array_from_slice(&src).expect("byte array from slice succeeds");

		let dest: std::result::Result<Vec<u8>, abos_jni::errors::Error> = env.convert_byte_array(java_byte_array);
		assert!(dest.is_ok());
		assert_eq!(dest.expect("test operation succeeds"), src);

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
