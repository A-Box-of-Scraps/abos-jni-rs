//! Wrapper tests using initialized mock interfaces, without starting a JVM.
#![allow(unsafe_code, reason = "Mock native tables exercise the wrapper's FFI boundaries.")]

use std::{
	ptr,
	sync::atomic::{AtomicUsize, Ordering},
};

use abos_jvmti::{Allocation, Env, Error, JvmtiVersion, sys};

static_assertions::assert_impl_all!(Env<'static>: Send, Sync);
static_assertions::assert_not_impl_any!(Allocation<'static, 'static>: Send, Sync, Clone, Copy);

fn with_env(table: &sys::jvmtiInterface_1_, test: impl FnOnce(Env<'_>)) {
	let mut raw: sys::jvmtiEnv = table;
	// SAFETY: The full initialized table and environment live until the closure returns.
	let env: Env<'_> =
		unsafe { Env::from_raw_with_version(&raw mut raw, JvmtiVersion::V25) }.expect("valid mock environment");
	test(env);
}

#[test]
fn rejects_null_environment_and_table() {
	assert_eq!(
		// SAFETY: Null is explicitly supported as an error case by the constructor.
		unsafe { Env::from_raw(ptr::null_mut()) }.expect_err("operation must fail"),
		Error::NullPointer
	);
	let mut raw: sys::jvmtiEnv = ptr::null();
	assert_eq!(
		// SAFETY: The pointer is readable; its null table is rejected.
		unsafe { Env::from_raw(&raw mut raw) }.expect_err("operation must fail"),
		Error::NullPointer
	);
	assert_eq!(
		// SAFETY: Null is explicitly supported as an error case by the VM constructor.
		unsafe { Env::from_raw_vm(ptr::null_mut()) }.expect_err("operation must fail"),
		Error::NullPointer
	);
}

#[test]
fn absent_functions_are_reported_without_calling_null() {
	with_env(&sys::jvmtiInterface_1_::default(), |env| {
		// SAFETY: The mock interface is alive; no native function is called.
		unsafe {
			assert_eq!(env.version(), JvmtiVersion::V25);
			assert_eq!(env.phase(), Err(Error::MissingFunction("GetPhase")));
			assert_eq!(
				env.allocate(1).expect_err("operation must fail"),
				Error::MissingFunction("Allocate")
			);
			assert_eq!(env.dispose(), Err(Error::MissingFunction("DisposeEnvironment")));
		}
	});
}

unsafe extern "system" fn version(_env: *mut sys::jvmtiEnv, value: *mut sys::jint) -> sys::jvmtiError {
	// SAFETY: The wrapper supplies a writable output slot.
	unsafe { *value = sys::JVMTI_VERSION };
	sys::JVMTI_ERROR_NONE
}

const unsafe extern "system" fn wrong_phase(_env: *mut sys::jvmtiEnv, _value: *mut sys::jvmtiPhase) -> sys::jvmtiError {
	sys::JVMTI_ERROR_WRONG_PHASE
}

const unsafe extern "system" fn unknown_error(_env: *mut sys::jvmtiEnv, _value: *mut sys::jint) -> sys::jvmtiError {
	0xfedc_ba98
}

#[test]
fn values_and_native_errors_are_preserved() {
	with_env(
		&sys::jvmtiInterface_1_ {
			GetVersionNumber: Some(version),
			GetPhase: Some(wrong_phase),
			..Default::default()
		},
		|env| {
			// SAFETY: The initialized mock functions accept this context and their output slots.
			unsafe {
				assert_eq!(env.version(), JvmtiVersion::V25);
				assert_eq!(env.phase(), Err(Error::Jvmti(sys::JVMTI_ERROR_WRONG_PHASE)));
			}
		},
	);
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(unknown_error),
		..Default::default()
	};
	let mut raw: sys::jvmtiEnv = &raw const table;
	assert_eq!(
		// SAFETY: The initialized mock returns an intentionally unknown integer error code.
		unsafe { Env::from_raw(&raw mut raw) }.expect_err("query fails"),
		Error::Jvmti(0xfedc_ba98)
	);
}

static FREED: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn allocate(
	_env: *mut sys::jvmtiEnv,
	size: sys::jlong,
	output: *mut *mut u8,
) -> sys::jvmtiError {
	let pointer: *mut u8 = match size {
		0 => ptr::null_mut(),
		16 => Box::into_raw(Box::new([0xa5_u8; 16])).cast(),
		_ => return sys::JVMTI_ERROR_OUT_OF_MEMORY,
	};
	// SAFETY: The wrapper supplies a writable output pointer.
	unsafe { *output = pointer };
	sys::JVMTI_ERROR_NONE
}

unsafe extern "system" fn deallocate(_env: *mut sys::jvmtiEnv, pointer: *mut u8) -> sys::jvmtiError {
	// SAFETY: Nonempty allocations from the mock allocator are owned Box<[u8; 16]>.
	drop(unsafe { Box::from_raw(pointer.cast::<[u8; 16]>()) });
	FREED.fetch_add(1, Ordering::SeqCst);
	sys::JVMTI_ERROR_NONE
}

#[test]
fn allocation_is_initialized_freed_once_and_can_transfer_ownership() {
	with_env(
		&sys::jvmtiInterface_1_ {
			Allocate: Some(allocate),
			Deallocate: Some(deallocate),
			..Default::default()
		},
		|env| {
			// SAFETY: Mock allocation and deallocation accept this context; all allocations
			// are freed before the local environment and its table go away.
			unsafe {
				let mut memory: Allocation<'_, '_> = env.allocate(16).expect("allocate");
				assert_eq!(&*memory, &[0; 16]);
				memory[0] = 42;
				assert_eq!(memory[0], 42);
				drop(memory);
				assert_eq!(FREED.load(Ordering::SeqCst), 1);
				let empty: Allocation<'_, '_> = env.allocate(0).expect("empty allocation");
				assert!(empty.is_empty());
				assert!(empty.into_raw().is_null());
				assert_eq!(FREED.load(Ordering::SeqCst), 1);
				let pointer: *mut u8 = env.allocate(16).expect("transfer allocation").into_raw();
				assert_eq!(FREED.load(Ordering::SeqCst), 1);
				deallocate(env.as_raw(), pointer);
				assert_eq!(FREED.load(Ordering::SeqCst), 2);
				assert_eq!(
					env.allocate(17).expect_err("operation must fail"),
					Error::Jvmti(sys::JVMTI_ERROR_OUT_OF_MEMORY)
				);
				assert_eq!(
					env.allocate(usize::MAX).expect_err("operation must fail"),
					Error::AllocationTooLarge
				);
			}
		},
	);
}

const unsafe extern "system" fn null_allocation(
	_env: *mut sys::jvmtiEnv,
	_size: sys::jlong,
	_output: *mut *mut u8,
) -> sys::jvmtiError {
	sys::JVMTI_ERROR_NONE
}

#[test]
fn rejects_null_nonempty_allocation_and_checks_deallocator_before_allocating() {
	with_env(
		&sys::jvmtiInterface_1_ {
			Allocate: Some(null_allocation),
			..Default::default()
		},
		|env| {
			assert_eq!(
				// SAFETY: No allocator is called because the deallocator is absent.
				unsafe { env.allocate(16) }.expect_err("operation must fail"),
				Error::MissingFunction("Deallocate")
			);
		},
	);
	with_env(
		&sys::jvmtiInterface_1_ {
			Allocate: Some(null_allocation),
			Deallocate: Some(deallocate),
			..Default::default()
		},
		|env| {
			assert_eq!(
				// SAFETY: The mock returns a null output deliberately; no memory is dereferenced.
				unsafe { env.allocate(16) }.expect_err("operation must fail"),
				Error::NullPointer
			);
		},
	);
}
