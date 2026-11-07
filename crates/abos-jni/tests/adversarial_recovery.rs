#![allow(
	unsafe_code,
	reason = "Shared test utilities contain JNI FFI helpers; scenarios use safe APIs."
)]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env,
	errors::{Error, JniError, Result},
	jni_sig, jni_str,
	objects::{JClassLoader, JObject, JObjectArray, JString, LoaderContext},
	refs::Reference,
};
use assert_matches::assert_matches;
use rusty_fork::rusty_fork_test;

fn assert_usable(env: &mut Env<'_>) -> Result<()> {
	assert!(!env.exception_check());
	let string: JString<'_> = env.new_string("recovered")?;
	let array: JObjectArray<'_> = env.new_object_array(1, jni_str!("java/lang/String"), &string)?;
	let array: JObjectArray<'_, JString<'_>> = env.cast_local::<JObjectArray<JString>>(array)?;
	let element: JString<'_> = array.get_element(env, 0)?;
	assert_eq!(element.try_to_string(env)?, "recovered");
	assert_eq!(
		env.call_method(&element, jni_str!("length"), jni_sig!("()I"), &[])?
			.i()?,
		9
	);
	Ok(())
}

fn assert_invalid_arguments<T: std::fmt::Debug>(result: Result<T>) {
	assert_matches!(
		result.expect_err("length or index must fit JNI"),
		Error::JniCall(JniError::InvalidArguments)
	);
}

rusty_fork_test! {
#[test]
fn object_array_index_failures_leave_the_environment_usable() {
	util::attach_current_thread(|env| {
		let original = env.new_string("original")?;
		for length in [0, 1] {
			let array = JObjectArray::<JString>::new(env, length, &original)?;
			assert_eq!(array.len(env)?, length);
			assert_matches!(array.get_element(env, length), Err(Error::IndexOutOfBounds));
			assert_usable(env)?;
			assert_matches!(array.set_element(env, length, JString::null()), Err(Error::IndexOutOfBounds));
			assert_usable(env)?;
			for index in [i32::MAX as usize + 1, usize::MAX] {
				assert_invalid_arguments(array.get_element(env, index));
				assert_usable(env)?;
				assert_invalid_arguments(array.set_element(env, index, JString::null()));
				assert_usable(env)?;
			}
			if length != 0 {
				assert_eq!(array.get_element(env, 0)?.try_to_string(env)?, "original");
				array.set_element(env, 0, JString::null())?;
				assert!(array.get_element(env, 0)?.is_null());
			}
		}
		Ok(())
	})
	.expect("object array bounds checks preserve references and exception state");
}

#[test]
fn primitive_array_regions_recover_from_invalid_bounds() {
	util::attach_current_thread(|env| {
		for length in [0, 1] {
			let array = env.new_int_array(length)?;
			assert_eq!(array.len(env)?, length);
			if length != 0 {
				array.set_region(env, 0, &[42])?;
			}
			let mut output = [99];
			for start in [-1, i32::try_from(length).expect("small array"), i32::MAX] {
				assert_matches!(array.get_region(env, start, &mut output), Err(Error::IndexOutOfBounds));
				assert_eq!(output, [99]);
				assert_usable(env)?;
				assert_matches!(array.set_region(env, start, &[7]), Err(Error::IndexOutOfBounds));
				assert_usable(env)?;
			}
			let end = i32::try_from(length).expect("small array");
			array.get_region(env, end, &mut [])?;
			array.set_region(env, end, &[])?;
			if length != 0 {
				array.get_region(env, 0, &mut output)?;
				assert_eq!(output, [42]);
				array.set_region(env, 0, &[7])?;
				array.get_region(env, 0, &mut output)?;
				assert_eq!(output, [7]);
			}
		}
		Ok(())
	})
	.expect("primitive region failures leave data unchanged and permit subsequent operations");
}

#[test]
fn oversized_lengths_are_rejected_without_allocating() {
	util::attach_current_thread(|env| {
		// These values cannot fit jsize, so rejection must not depend on available heap space.
		for length in [i32::MAX as usize + 1, usize::MAX] {
			assert_invalid_arguments(JObjectArray::<JString>::new(env, length, JString::null()));
			assert_usable(env)?;
			macro_rules! check_primitive {
				($constructor:ident) => {
					assert_invalid_arguments(env.$constructor(length));
					assert_usable(env)?;
				};
			}
			check_primitive!(new_boolean_array);
			check_primitive!(new_byte_array);
			check_primitive!(new_char_array);
			check_primitive!(new_short_array);
			check_primitive!(new_int_array);
			check_primitive!(new_long_array);
			check_primitive!(new_float_array);
			check_primitive!(new_double_array);
		}
		Ok(())
	})
	.expect("oversized lengths fail before allocation");
}

#[test]
fn negative_object_array_lengths_leave_a_catchable_exception() {
	util::attach_current_thread(|env| {
		for length in [-1, i32::MIN] {
			assert_matches!(
				env.new_object_array(length, jni_str!("java/lang/String"), JObject::null()),
				Err(Error::JavaException)
			);
			assert!(env.exception_check());
			assert_matches!(env.exception_catch(), Err(Error::CaughtJavaException { name, .. }) => {
				assert_eq!(name, "java.lang.NegativeArraySizeException");
			});
			assert_usable(env)?;
		}
		Ok(())
	})
	.expect("negative sizes can be handled without poisoning the environment");
}

#[test]
fn pending_exceptions_block_operations_without_losing_the_original_exception() {
	util::attach_current_thread(|env| {
		let string = env.new_string("original")?;
		let array = JObjectArray::<JString>::new(env, 1, &string)?;
		let ints = env.new_int_array(1)?;
		ints.set_region(env, 0, &[42])?;
		// Re-throw before each operation so every case independently verifies preservation and recovery.
		for operation in 0..9 {
			assert_matches!(env.throw("pending sentinel"), Err(Error::JavaException));
			let original_exception = env.exception_occurred().expect("pending exception");
			let mut output = [99];
			let result = match operation {
				0 => env.new_string("blocked").map(|_| ()),
				1 => env.new_object_array(1, jni_str!("java/lang/String"), &string).map(|_| ()),
				2 => array.get_element(env, 0).map(|_| ()),
				3 => array.set_element(env, 0, JString::null()),
				4 => env.call_method(&string, jni_str!("length"), jni_sig!("()I"), &[]).map(|_| ()),
				5 => env.new_cast_local_ref::<JString>(&string).map(|_| ()),
				6 => ints.get_region(env, 0, &mut output),
				7 => ints.set_region(env, 0, &[7]),
				_ => env.get_static_field(jni_str!("java/lang/Integer"), jni_str!("MAX_VALUE"), jni_sig!("I")).map(|_| ()),
			};
			assert_matches!(result, Err(Error::JavaException), "operation {operation}");
			assert!(env.exception_check());
			let remaining_exception = env.exception_occurred().expect("original exception still pending");
			assert_matches!(env.exception_catch(), Err(Error::CaughtJavaException { name, msg, .. }) => {
				assert_eq!(name, "java.lang.RuntimeException");
				assert_eq!(msg, "pending sentinel");
			});
			assert!(env.is_same_object(&original_exception, &remaining_exception)?);
			assert_eq!(output, [99]);
			assert_eq!(array.get_element(env, 0)?.try_to_string(env)?, "original");
			ints.get_region(env, 0, &mut output)?;
			assert_eq!(output, [42]);
			assert_usable(env)?;
		}
		Ok(())
	})
	.expect("pending exceptions survive blocked calls and can be handled before reuse");
}

#[test]
fn class_loading_preserves_pending_exceptions_for_every_loader_context() {
	util::attach_current_thread(|env| {
		let object = env.new_string("candidate")?;
		let loader = JClassLoader::get_system_class_loader(env)?;
		let contexts = [LoaderContext::None, LoaderContext::FromObject(object.as_ref()), LoaderContext::Loader(&loader)];
		for context in contexts {
			for name in [jni_str!("java.lang.String"), jni_str!("missing.AdversarialFixture")] {
				assert_matches!(env.throw("lookup sentinel"), Err(Error::JavaException));
				let original = env.exception_occurred().expect("pending exception");
				assert_matches!(context.load_class(env, name, false), Err(Error::JavaException));
				let remaining = env.exception_occurred().expect("lookup must preserve the exception");
				assert_matches!(env.exception_catch(), Err(Error::CaughtJavaException { name, msg, .. }) => {
					assert_eq!(name, "java.lang.RuntimeException");
					assert_eq!(msg, "lookup sentinel");
				});
				assert!(env.is_same_object(&original, &remaining)?);
				let class = context.load_class(env, jni_str!("java.lang.String"), false)?;
				assert!(env.is_instance_of(&object, &class)?);
				assert_usable(env)?;
			}
		}
		Ok(())
	})
	.expect("class lookup only handles exceptions raised by that lookup");
}

#[test]
fn explicit_frame_promotion_and_global_thread_transfers_remain_valid() {
	util::attach_current_thread(|env| {
		let promoted = env.with_local_frame_returning_local::<_, JString, _>(1, |inner| {
			inner.new_string("promoted")
		})?;
		assert_eq!(promoted.try_to_string(env)?, "promoted");
		let global = env.with_local_frame(1, |inner| {
			let string = inner.new_string("global")?;
			inner.new_global_ref(string)
		})?;
		let moved = env.new_global_ref(&global)?;
		std::thread::scope(|scope| {
			scope.spawn(move || {
				util::attach_current_thread(|env| {
					assert_eq!(moved.try_to_string(env)?, "global");
					Ok(())
				}).expect("global reference can move to another attached thread");
			});
			scope.spawn(|| {
				util::attach_current_thread(|env| {
					assert_eq!(global.try_to_string(env)?, "global");
					Ok(())
				}).expect("global reference can be shared with another attached thread");
			});
		});
		assert_eq!(global.try_to_string(env)?, "global");
		assert_usable(env)?;
		Ok(())
	})
	.expect("explicit promotion permits valid frame and thread transfers");
}
}
