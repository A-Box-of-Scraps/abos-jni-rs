//! Returned allocations are tracked by originating mock environments.
#![allow(
	unsafe_code,
	reason = "Tests adopt mock JVMTI allocations and call native interfaces."
)]

use std::{ffi::c_char, ptr};

use abos_jni::jni_str;
use abos_jvmti::{Allocation, Error, JvmtiArray, JvmtiString, JvmtiStringList, LocalVariableTable, Result, sys};

#[path = "returned_data/nested.rs"]
mod nested;
#[path = "returned_data/support.rs"]
mod support;
use support::{Mode, with_env, with_table};

static_assertions::assert_not_impl_any!(JvmtiArray<'static, 'static, u8>: Send, Sync, Clone, Copy);
static_assertions::assert_not_impl_any!(JvmtiString<'static, 'static>: Send, Sync, Clone, Copy);
static_assertions::assert_not_impl_any!(JvmtiStringList<'static, 'static>: Send, Sync, Clone, Copy);
static_assertions::assert_not_impl_any!(LocalVariableTable<'static, 'static>: Send, Sync, Clone, Copy);
static_assertions::assert_not_impl_any!(Allocation<'static, 'static>: Send, Sync, Clone, Copy);

#[test]
fn arrays_adopt_initialized_data_and_preserve_raw_parts() {
	with_env(|context, env| {
		let pointer: *mut i64 = context.allocate(&[10_i64, 20]);
		// SAFETY: Exclusive initialized allocation from this environment; owner remains local.
		let mut array: JvmtiArray<'_, '_, i64> = unsafe { JvmtiArray::from_raw(&env, pointer, 2) }.expect("array");
		assert_eq!(&*array, &[10, 20]);
		array[0] = 30;
		let (raw, length): (*mut i64, usize) = array.into_raw();
		assert_eq!((raw, length), (pointer, 2));
		assert!(context.calls.borrow().is_empty());
		// SAFETY: The previous owner transferred this exact allocation, still live.
		let array: JvmtiArray<'_, '_, i64> = unsafe { JvmtiArray::from_raw(&env, raw, length) }.expect("readopt");
		assert_eq!(array[0], 30);
		array.free().expect("explicit cleanup");
		assert_eq!(&*context.calls.borrow(), &[pointer as usize]);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn empty_arrays_preserve_null_and_nonnull_ownership() {
	with_env(|context, env| {
		// SAFETY: Empty null arrays contain no data and require no cleanup.
		let array: JvmtiArray<'_, '_, u64> = unsafe { JvmtiArray::from_raw(&env, ptr::null_mut(), 0) }.expect("empty");
		assert!(array.is_empty());
		assert_eq!(array.into_raw(), (ptr::null_mut(), 0));
		let pointer: *mut u64 = context.allocate(&[]);
		// SAFETY: Even with zero elements the pointer owns a distinct allocation.
		let array: JvmtiArray<'_, '_, u64> = unsafe { JvmtiArray::from_raw(&env, pointer, 0) }.expect("nonnull empty");
		assert!(array.is_empty());
		drop(array);
		assert_eq!(&*context.calls.borrow(), &[pointer as usize]);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn adopting_invalid_shapes_does_not_take_ownership() {
	with_env(|context, env| {
		// SAFETY: Invalid shapes are rejected before dereferencing the null pointer.
		unsafe {
			assert_eq!(
				JvmtiArray::<u64>::from_raw(&env, ptr::null_mut(), 1).expect_err("null"),
				Error::NullPointer
			);
			assert_eq!(
				JvmtiArray::<u64>::from_raw(&env, ptr::null_mut(), usize::MAX).expect_err("size"),
				Error::AllocationTooLarge
			);
			assert_eq!(
				JvmtiString::from_raw(&env, ptr::null_mut()).expect_err("null string"),
				Error::NullPointer
			);
		}
		assert!(context.calls.borrow().is_empty());
	});
}

#[test]
fn embedded_jni_references_are_not_deallocated_or_adopted() {
	with_env(|context, env| {
		let reference: sys::jobject = ptr::dangling_mut();
		let pointer: *mut sys::jobject = context.allocate(&[reference]);
		// SAFETY: The pointer array is valid owned storage; its element is never dereferenced as JNI.
		let array: JvmtiArray<'_, '_, sys::jobject> = unsafe { JvmtiArray::from_raw(&env, pointer, 1) }.expect("refs");
		assert_eq!(array[0], reference);
		drop(array);
		assert_eq!(&*context.calls.borrow(), &[pointer as usize]);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn cleanup_uses_the_originating_environment_even_with_multiple_connections() {
	with_env(|first, env1| {
		with_env(|second, env2| {
			let one: *mut i32 = first.allocate(&[1]);
			let two: *mut i32 = second.allocate(&[2]);
			// SAFETY: Each allocation is adopted only by its own live environment.
			unsafe {
				drop(JvmtiArray::from_raw(&env2, two, 1).expect("second"));
				drop(JvmtiArray::from_raw(&env1, one, 1).expect("first"));
			}
			assert_eq!(&*first.calls.borrow(), &[one as usize]);
			assert_eq!(&*second.calls.borrow(), &[two as usize]);
		});
	});
}

#[test]
fn string_decodes_modified_utf8_and_transfers_ownership() {
	with_env(|context, env| {
		// SAFETY: The mock supplies owned, terminated MUTF-8 and cleanup stays in context.
		let string: JvmtiString<'_, '_> = unsafe { env.system_property(jni_str!("key")) }.expect("property");
		assert_eq!(string.to_str(), "value\0😀");
		let raw: *mut c_char = string.into_raw();
		assert!(context.calls.borrow().is_empty());
		// SAFETY: into_raw transferred the live string allocation back to this caller.
		unsafe { JvmtiString::from_raw(&env, raw) }
			.expect("readopt")
			.free()
			.expect("free");
		let empty: *mut c_char = context.allocate(b"\0").cast();
		// SAFETY: A terminated empty string still owns a one-byte allocation.
		let string: JvmtiString<'_, '_> = unsafe { JvmtiString::from_raw(&env, empty) }.expect("empty string");
		assert_eq!(string.to_bytes(), []);
		drop(string);
		assert_eq!(&*context.calls.borrow(), &[raw as usize, empty as usize]);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn flat_outputs_handle_empty_invalid_and_failed_results() {
	for mode in [
		Mode::Normal,
		Mode::Empty,
		Mode::EmptyAllocated,
		Mode::Negative,
		Mode::NativeError,
		Mode::NullOutput,
	] {
		with_env(|context, env| {
			context.mode.set(mode);
			// SAFETY: The mock ignores the method, returning the chosen response shape.
			let result: Result<JvmtiArray<'_, '_, sys::jvmtiLineNumberEntry>> =
				unsafe { env.line_number_table(ptr::null_mut()) };
			match mode {
				Mode::Normal => {
					let array: JvmtiArray<'_, '_, sys::jvmtiLineNumberEntry> = result.expect("table");
					assert_eq!(array[0].line_number, 7);
					drop(array);
				}
				Mode::Empty | Mode::EmptyAllocated => {
					let array: JvmtiArray<'_, '_, sys::jvmtiLineNumberEntry> = result.expect("empty");
					assert!(array.is_empty());
					drop(array);
				}
				Mode::Negative => assert_eq!(result.expect_err("count"), Error::InvalidCount(-1)),
				Mode::NativeError => {
					assert_eq!(result.expect_err("error"), Error::Jvmti(sys::JVMTI_ERROR_OUT_OF_MEMORY));
				}
				Mode::NullOutput => assert_eq!(result.expect_err("null"), Error::NullPointer),
				Mode::NullChild => unreachable!(),
			}
			assert_eq!(
				context.calls.borrow().len(),
				usize::from(matches!(mode, Mode::Normal | Mode::EmptyAllocated | Mode::Negative))
			);
			assert_eq!(context.live(), 0);
		});
	}
}

#[test]
fn explicit_cleanup_reports_failure_without_retry_and_drop_ignores_failure() {
	for explicit in [false, true] {
		with_env(|context, env| {
			context.fail_at.set(Some(1));
			let pointer: *mut u8 = context.allocate(&[42]);
			// SAFETY: Live exclusive mock allocation; failing cleanup is intentional.
			let array: JvmtiArray<'_, '_, u8> = unsafe { JvmtiArray::from_raw(&env, pointer, 1) }.expect("array");
			if explicit {
				assert_eq!(array.free(), Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)));
			} else {
				drop(array);
			}
			assert_eq!(&*context.calls.borrow(), &[pointer as usize]);
			assert_eq!(context.live(), 1);
		});
	}
}

#[test]
fn missing_deallocator_is_checked_before_querying_any_outputs() {
	with_table(&sys::jvmtiInterface_1_::default(), |_, env| {
		// SAFETY: No native function is present and no pointer is dereferenced.
		unsafe {
			assert_eq!(
				env.system_properties().expect_err("missing"),
				Error::MissingFunction("GetSystemProperties")
			);
			assert_eq!(
				env.system_property(jni_str!("key")).expect_err("missing"),
				Error::MissingFunction("GetSystemProperty")
			);
			assert_eq!(
				env.line_number_table(ptr::null_mut()).expect_err("missing"),
				Error::MissingFunction("GetLineNumberTable")
			);
			assert_eq!(
				env.local_variable_table(ptr::null_mut()).expect_err("missing"),
				Error::MissingFunction("GetLocalVariableTable")
			);
			assert_eq!(
				JvmtiArray::<u8>::from_raw(&env, ptr::null_mut(), 0).expect_err("missing"),
				Error::MissingFunction("Deallocate")
			);
		}
	});
}

#[test]
fn queries_do_not_allocate_when_cleanup_is_unavailable() {
	let mut table: sys::jvmtiInterface_1_ = support::table();
	table.Deallocate = None;
	with_table(&table, |context, env| {
		// SAFETY: Every operation should reject the interface before calling the native mock.
		unsafe {
			let error: Error = Error::MissingFunction("Deallocate");
			assert_eq!(env.system_properties().expect_err("deallocator"), error);
			assert_eq!(env.system_property(jni_str!("key")).expect_err("deallocator"), error);
			assert_eq!(env.line_number_table(ptr::null_mut()).expect_err("deallocator"), error);
			assert_eq!(
				env.local_variable_table(ptr::null_mut()).expect_err("deallocator"),
				error
			);
		}
		assert_eq!(context.live(), 0);
		assert!(context.calls.borrow().is_empty());
	});
}

#[test]
fn bytecodes_own_only_successful_outputs_and_free_once() {
	for mode in [
		Mode::Normal,
		Mode::Empty,
		Mode::EmptyAllocated,
		Mode::Negative,
		Mode::NativeError,
		Mode::NullOutput,
	] {
		with_env(|context, env| {
			context.mode.set(mode);
			// SAFETY: Mock ignores the method and returns tracked allocations or undefined error outputs.
			let result: Result<JvmtiArray<'_, '_, u8>> = unsafe { env.bytecodes(ptr::null_mut()) };
			match mode {
				Mode::Normal => {
					let bytes: JvmtiArray<'_, '_, u8> = result.expect("bytecodes");
					assert_eq!(&*bytes, &[0x2a, 0xb1]);
					bytes.free().expect("cleanup");
				}
				Mode::Empty | Mode::EmptyAllocated => assert!(result.expect("empty").is_empty()),
				Mode::Negative => assert_eq!(result.expect_err("count"), Error::InvalidCount(-1)),
				Mode::NativeError => assert_eq!(
					result.expect_err("native"),
					Error::Jvmti(sys::JVMTI_ERROR_OUT_OF_MEMORY)
				),
				Mode::NullOutput => assert_eq!(result.expect_err("pointer"), Error::NullPointer),
				Mode::NullChild => unreachable!(),
			}
			assert_eq!(
				context.calls.borrow().len(),
				usize::from(matches!(mode, Mode::Normal | Mode::EmptyAllocated | Mode::Negative))
			);
			assert_eq!(context.live(), 0);
		});
	}
}
