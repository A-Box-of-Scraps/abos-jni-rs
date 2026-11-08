use std::{
	ffi::{c_double, c_int},
	ptr,
};

use abos_jni_sys as sys;

type MethodV =
	unsafe extern "system" fn(*mut sys::JNIEnv, sys::jobject, sys::jmethodID, sys::va_list<'_>) -> sys::jdouble;

unsafe extern "C" {
	fn abos_jni_probe_boolean(
		value: sys::jboolean,
		copy: *mut sys::jboolean,
		slot: *mut sys::jvalue,
		callback: extern "system" fn(sys::jboolean) -> sys::jboolean,
	) -> sys::jboolean;
	fn abos_jni_probe_table() -> *const sys::JNINativeInterface_;
	fn abos_jni_probe_call_rust(callback: MethodV) -> sys::jdouble;
}

extern "system" fn echo_boolean(value: sys::jboolean) -> sys::jboolean {
	value
}

#[test]
fn boolean_arguments_returns_pointers_and_union_preserve_all_bits() {
	// This C-only fixture tests the raw unsigned-char ABI, not Java boolean semantics.
	for value in 0..=u8::MAX {
		let mut copy = 0;
		let mut slot = sys::jvalue { z: 0 };
		// SAFETY: Both output pointers are live, and the callback has the C-declared ABI.
		unsafe {
			assert_eq!(
				abos_jni_probe_boolean(value, &raw mut copy, &raw mut slot, echo_boolean),
				value
			);
			assert_eq!(copy, value);
			assert_eq!(slot.z, value);
		}
	}
}

unsafe extern "system" fn sum_in_rust(
	_env: *mut sys::JNIEnv,
	_object: sys::jobject,
	_method: sys::jmethodID,
	mut args: sys::va_list<'_>,
) -> sys::jdouble {
	let mut sum = 0.0;
	for _ in 0..12 {
		// SAFETY: The C fixture supplies twelve alternating int/double pairs.
		unsafe {
			sum += f64::from(args.next_arg::<c_int>());
			sum += args.next_arg::<c_double>();
		}
	}
	sum
}

unsafe extern "system" fn forward_to_c(
	env: *mut sys::JNIEnv,
	object: sys::jobject,
	method: sys::jmethodID,
	args: sys::va_list<'_>,
) -> sys::jdouble {
	// SAFETY: Only the populated mock slot is read. Its implementation ignores the JNI
	// handles and consumes the fixture's twelve pairs. Cloning preserves the Rust cursor.
	unsafe {
		let call = (*abos_jni_probe_table()).v1_1.CallDoubleMethodV;
		call(env, object, method, args.clone()) + sum_in_rust(env, object, method, args)
	}
}

unsafe extern "C" fn rust_variadic(_unused: c_int, args: ...) -> sys::jdouble {
	// SAFETY: The test supplies the same argument sequence as the C fixture.
	unsafe { forward_to_c(ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), args) }
}

#[test]
fn c_va_list_can_be_consumed_in_rust() {
	// SAFETY: The callback accepts the exact sequence supplied by the C fixture.
	assert_eq!(unsafe { abos_jni_probe_call_rust(sum_in_rust) }, 150.0);
}

#[test]
fn c_va_list_can_be_cloned_and_forwarded_through_a_jni_slot() {
	// SAFETY: The callback and mock table share the fixture's argument contract.
	assert_eq!(unsafe { abos_jni_probe_call_rust(forward_to_c) }, 300.0);
}

#[test]
fn rust_va_list_can_be_passed_through_a_jni_slot() {
	// SAFETY: Twelve alternating C int/double pairs match both consumers.
	let result = unsafe {
		rust_variadic(
			0, 1_i32, 0.5_f64, 2_i32, 1.5_f64, 3_i32, 2.5_f64, 4_i32, 3.5_f64, 5_i32, 4.5_f64, 6_i32, 5.5_f64, 7_i32,
			6.5_f64, 8_i32, 7.5_f64, 9_i32, 8.5_f64, 10_i32, 9.5_f64, 11_i32, 10.5_f64, 12_i32, 11.5_f64,
		)
	};
	assert_eq!(result, 300.0);
}
