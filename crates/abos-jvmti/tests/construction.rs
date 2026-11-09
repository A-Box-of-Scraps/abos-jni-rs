//! Environment negotiation and cleanup using an initialized mock JNI VM.
#![allow(unsafe_code, reason = "Mock JVM entry points exercise native constructors.")]

use std::{cell::Cell, ffi::c_void, ptr};

use abos_jvmti::{Env, Error, JvmtiVersion, sys};

#[repr(C)]
struct MockEnv {
	table: sys::jvmtiEnv,
	actual: sys::jint,
	query_error: sys::jvmtiError,
	disposed: Cell<usize>,
}

#[repr(C)]
struct MockVm {
	table: sys::JavaVM,
	env: MockEnv,
	calls: Cell<usize>,
	requested: Cell<sys::jint>,
	status: sys::jint,
}

unsafe extern "system" fn get_env(vm: *mut sys::JavaVM, output: *mut *mut c_void, version: sys::jint) -> sys::jint {
	// SAFETY: MockVm is repr(C) and its VM pointer is the first field.
	let mock: &mut MockVm = unsafe { &mut *vm.cast::<MockVm>() };
	mock.calls.set(mock.calls.get() + 1);
	mock.requested.set(version);
	if mock.status == sys::JNI_OK {
		// SAFETY: The constructor supplies a writable output slot.
		unsafe { *output = (&raw mut mock.env.table).cast() };
	}
	mock.status
}

unsafe extern "system" fn get_version(raw: *mut sys::jvmtiEnv, output: *mut sys::jint) -> sys::jvmtiError {
	// SAFETY: MockEnv is repr(C) and the interface pointer is its first field.
	let mock: &MockEnv = unsafe { &*raw.cast::<MockEnv>() };
	// SAFETY: The constructor supplies a writable version output.
	unsafe { *output = mock.actual };
	mock.query_error
}

unsafe extern "system" fn dispose(raw: *mut sys::jvmtiEnv) -> sys::jvmtiError {
	// SAFETY: The mock environment is alive throughout constructor cleanup.
	let mock: &MockEnv = unsafe { &*raw.cast::<MockEnv>() };
	mock.disposed.set(mock.disposed.get() + 1);
	sys::JVMTI_ERROR_NONE
}

const unsafe extern "system" fn unused_vm(_vm: *mut sys::JavaVM) -> sys::jint {
	sys::JNI_ERR
}
const unsafe extern "system" fn unused_attach(
	_vm: *mut sys::JavaVM,
	_output: *mut *mut c_void,
	_args: *mut c_void,
) -> sys::jint {
	sys::JNI_ERR
}

fn with_vm(test: impl FnOnce(&mut MockVm)) {
	let invocation = sys::JNIInvokeInterface_ {
		reserved: sys::JNIInvokeInterface__reserved {
			reserved0: ptr::null_mut(),
			reserved1: ptr::null_mut(),
			reserved2: ptr::null_mut(),
			DestroyJavaVM: unused_vm,
			AttachCurrentThread: unused_attach,
			DetachCurrentThread: unused_vm,
			GetEnv: get_env,
			AttachCurrentThreadAsDaemon: unused_attach,
		},
	};
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(get_version),
		DisposeEnvironment: Some(dispose),
		..Default::default()
	};
	let mut mock = MockVm {
		table: &raw const invocation,
		env: MockEnv {
			table: &raw const table,
			actual: sys::JVMTI_VERSION,
			query_error: sys::JVMTI_ERROR_NONE,
			disposed: Cell::new(0),
		},
		calls: Cell::new(0),
		requested: Cell::new(0),
		status: sys::JNI_OK,
	};
	test(&mut mock);
}

#[test]
fn default_requests_baseline_once_and_retains_actual_version() {
	with_vm(|mock| {
		// SAFETY: The mock VM and environment remain alive for the handle's use.
		let env: Env<'_> = unsafe { Env::from_raw_vm(&raw mut mock.table) }.expect("create");
		assert_eq!(mock.requested.get(), JvmtiVersion::MINIMUM.as_raw());
		assert_eq!(mock.calls.get(), 1);
		assert_eq!(env.version(), JvmtiVersion::V25);
		assert_eq!(mock.env.disposed.get(), 0);
		// SAFETY: There are no allocations or callbacks and this is the last use.
		unsafe { env.dispose() }.expect("dispose");
		assert_eq!(mock.env.disposed.get(), 1);
	});
}

#[test]
fn explicit_requests_are_not_silently_downgraded_or_retried() {
	with_vm(|mock| {
		mock.status = sys::JNI_EVERSION;
		// SAFETY: The mock VM deliberately rejects the requested version.
		let error: Error =
			unsafe { Env::from_raw_vm_with_version(&raw mut mock.table, JvmtiVersion::V21) }.expect_err("unsupported");
		assert_eq!(error, Error::Jni(sys::JNI_EVERSION));
		assert_eq!(mock.requested.get(), sys::JVMTI_VERSION_21);
		assert_eq!(mock.calls.get(), 1);
		assert_eq!(mock.env.disposed.get(), 0);
	});
	with_vm(|mock| {
		// SAFETY: The mock supplies a compatible newer environment.
		let env: Env<'_> =
			unsafe { Env::from_raw_vm_with_version(&raw mut mock.table, JvmtiVersion::V11) }.expect("explicit version");
		assert_eq!(mock.requested.get(), sys::JVMTI_VERSION_11);
		assert_eq!(env.version(), JvmtiVersion::V25);
		assert_eq!(mock.calls.get(), 1);
	});
}

#[test]
fn validation_failures_dispose_new_environments() {
	for actual in [sys::JVMTI_VERSION_1_1, sys::JNI_VERSION_1_8, sys::JVMTI_VERSION_11] {
		with_vm(|mock| {
			mock.env.actual = actual;
			// SAFETY: The initialized mock deliberately reports an incompatible version.
			let result: abos_jvmti::Result<Env<'_>> =
				unsafe { Env::from_raw_vm_with_version(&raw mut mock.table, JvmtiVersion::V21) };
			assert!(result.is_err());
			assert_eq!(mock.calls.get(), 1);
			assert_eq!(mock.env.disposed.get(), 1);
		});
	}
	with_vm(|mock| {
		mock.env.query_error = sys::JVMTI_ERROR_INTERNAL;
		// SAFETY: The mock version query deliberately fails.
		let error: Error = unsafe { Env::from_raw_vm(&raw mut mock.table) }.expect_err("query failure");
		assert_eq!(error, Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
		assert_eq!(mock.env.disposed.get(), 1);
	});
}

#[test]
fn too_old_requests_are_rejected_without_creating_an_environment() {
	with_vm(|mock| {
		let old: JvmtiVersion = JvmtiVersion::from_raw(sys::JVMTI_VERSION_1_1).expect("valid identifier");
		// SAFETY: The VM is valid, although the wrapper rejects this before calling it.
		let error: Error = unsafe { Env::from_raw_vm_with_version(&raw mut mock.table, old) }.expect_err("old version");
		assert_eq!(
			error,
			Error::UnsupportedVersion {
				required: JvmtiVersion::MINIMUM,
				actual: old
			}
		);
		assert_eq!(mock.calls.get(), 0);
	});
}
