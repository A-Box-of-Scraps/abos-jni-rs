//! Version boundaries and callback registration without a real JVM.
#![allow(unsafe_code, reason = "Mock interfaces exercise version-gated FFI calls.")]

use std::{cell::Cell, ptr};

use abos_jvmti::{Env, Error, JvmtiVersion, sys};

#[test]
fn version_identifiers_validate_interface_bits_and_preserve_future_versions() {
	let version: JvmtiVersion = JvmtiVersion::from_raw(0x301b_0203).expect("future version");
	assert_eq!((version.major(), version.minor(), version.micro()), (27, 2, 3));
	assert_eq!(version.as_raw(), 0x301b_0203);
	assert!(version > JvmtiVersion::V25);
	assert_eq!(version.to_string(), "27.2.3");
	for raw in [sys::JNI_VERSION_1_8, -1, 0x4001_0200, i32::MIN | sys::JVMTI_VERSION] {
		assert_eq!(JvmtiVersion::from_raw(raw), Err(Error::InvalidVersion(raw)));
	}
	let patch: JvmtiVersion = JvmtiVersion::from_raw(sys::JVMTI_VERSION_1_2 + 1).expect("patch");
	assert!(JvmtiVersion::V1_2 < patch && patch < JvmtiVersion::V9);
}

#[repr(C)]
struct Context {
	table: sys::jvmtiEnv,
	version: sys::jint,
	queries: Cell<usize>,
	callback_bytes: Cell<sys::jint>,
	capability_calls: Cell<usize>,
}

unsafe extern "system" fn query_version(raw: *mut sys::jvmtiEnv, output: *mut sys::jint) -> sys::jvmtiError {
	// SAFETY: Context is repr(C) with the environment pointer as its first field.
	let context: &Context = unsafe { &*raw.cast::<Context>() };
	context.queries.set(context.queries.get() + 1);
	// SAFETY: The constructor supplies a writable output slot.
	unsafe { *output = context.version };
	sys::JVMTI_ERROR_NONE
}

unsafe extern "system" fn register_callbacks(
	raw: *mut sys::jvmtiEnv,
	_callbacks: *const sys::jvmtiEventCallbacks,
	bytes: sys::jint,
) -> sys::jvmtiError {
	// SAFETY: The context lives through the mock call.
	let context: &Context = unsafe { &*raw.cast::<Context>() };
	context.callback_bytes.set(bytes);
	sys::JVMTI_ERROR_NONE
}

unsafe extern "system" fn reject_capabilities(
	raw: *mut sys::jvmtiEnv,
	_caps: *const sys::jvmtiCapabilities,
) -> sys::jvmtiError {
	// SAFETY: The context lives through the mock call.
	let context: &Context = unsafe { &*raw.cast::<Context>() };
	context.capability_calls.set(context.capability_calls.get() + 1);
	sys::JVMTI_ERROR_NOT_AVAILABLE
}

const unsafe extern "system" fn thread_callback(
	_env: *mut sys::jvmtiEnv,
	_jni: *mut sys::JNIEnv,
	_thread: sys::jthread,
) {
}

const fn context(table: *const sys::jvmtiInterface_1_, version: JvmtiVersion) -> Context {
	Context {
		table,
		version: version.as_raw(),
		queries: Cell::new(0),
		callback_bytes: Cell::new(0),
		capability_calls: Cell::new(0),
	}
}

#[test]
fn baseline_table_is_shorter_than_latest_and_version_is_cached() {
	// Deliberately allocate only 155 slots, not the latest 156-slot table.
	let table: Box<sys::jvmtiInterface_1__1_2> = Box::new(sys::jvmtiInterface_1__1_2 {
		GetVersionNumber: Some(query_version),
		SetEventCallbacks: Some(register_callbacks),
		..Default::default()
	});
	let mut context: Context = context((&raw const *table).cast(), JvmtiVersion::V1_2);
	// SAFETY: The baseline table is initialized and both it and the context outlive env.
	let env: Env<'_> = unsafe { Env::from_raw(&raw mut context.table) }.expect("baseline environment");
	assert_eq!(env.version(), JvmtiVersion::V1_2);
	assert_eq!(env.version(), JvmtiVersion::V1_2);
	assert_eq!(context.queries.get(), 1);
	// SAFETY: The mock accepts the readable, empty callback table.
	unsafe { env.set_event_callbacks(&sys::jvmtiEventCallbacks::default()) }.expect("register baseline callbacks");
	assert_eq!(
		usize::try_from(context.callback_bytes.get()).expect("positive size"),
		35 * size_of::<*const ()>()
	);
	// SAFETY: The cached version is this environment's actual version.
	let borrowed: Env<'_> = unsafe { Env::from_raw_with_version(env.as_raw(), env.version()) }.expect("no query");
	assert_eq!(borrowed.version(), env.version());
	assert_eq!(context.queries.get(), 1);
}

#[test]
fn callbacks_are_sized_for_each_environment_and_new_events_are_rejected() {
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(query_version),
		SetEventCallbacks: Some(register_callbacks),
		..Default::default()
	};
	for (version, slots) in [
		(JvmtiVersion::V1_2, 35),
		(JvmtiVersion::V9, 35),
		(JvmtiVersion::V11, 37),
		(JvmtiVersion::V19, 37),
		(JvmtiVersion::V21, 39),
		(JvmtiVersion::V25, 39),
	] {
		let mut context: Context = context(&raw const table, version);
		// SAFETY: The complete initialized mock supports this version and stays alive.
		let env: Env<'_> = unsafe { Env::from_raw(&raw mut context.table) }.expect("environment");
		// SAFETY: The mock accepts an empty callback table.
		unsafe { env.set_event_callbacks(&sys::jvmtiEventCallbacks::default()) }.expect("callbacks");
		assert_eq!(
			usize::try_from(context.callback_bytes.get()).expect("positive size"),
			slots * size_of::<*const ()>()
		);
		let callbacks = sys::jvmtiEventCallbacks {
			VirtualThreadStart: Some(thread_callback),
			..Default::default()
		};
		// SAFETY: The mock only records size; this callback never unwinds.
		let result: abos_jvmti::Result<()> = unsafe { env.set_event_callbacks(&callbacks) };
		if version < JvmtiVersion::V21 {
			assert_eq!(
				result,
				Err(Error::UnsupportedVersion {
					required: JvmtiVersion::V21,
					actual: version
				})
			);
			assert_eq!(
				// SAFETY: The version guard rejects this before any native event call.
				unsafe { env.set_event_enabled(sys::JVMTI_EVENT_VIRTUAL_THREAD_START, true) },
				result
			);
		} else {
			result.expect("virtual-thread callbacks supported");
		}
	}
}

#[test]
fn capability_version_support_is_distinct_from_vm_availability() {
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(query_version),
		AddCapabilities: Some(reject_capabilities),
		..Default::default()
	};
	for version in [JvmtiVersion::V1_2, JvmtiVersion::V21] {
		let mut context: Context = context(&raw const table, version);
		// SAFETY: The table and context remain valid through these calls.
		let env: Env<'_> = unsafe { Env::from_raw(&raw mut context.table) }.expect("environment");
		let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		caps.set_can_support_virtual_threads(true);
		// SAFETY: The capabilities have no reserved bits and the mock accepts this context.
		let result: abos_jvmti::Result<()> = unsafe { env.add_capabilities(&caps) };
		if version < JvmtiVersion::V21 {
			assert_eq!(
				result,
				Err(Error::UnsupportedVersion {
					required: JvmtiVersion::V21,
					actual: version
				})
			);
			assert_eq!(context.capability_calls.get(), 0);
		} else {
			assert_eq!(result, Err(Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE)));
			assert_eq!(context.capability_calls.get(), 1);
		}
	}
}

#[test]
fn constructors_reject_missing_queries_invalid_identifiers_and_old_versions() {
	let table: sys::jvmtiInterface_1_ = sys::jvmtiInterface_1_::default();
	let mut raw: sys::jvmtiEnv = &raw const table;
	assert_eq!(
		// SAFETY: The initialized table deliberately lacks the version query function.
		unsafe { Env::from_raw(&raw mut raw) }.expect_err("missing query"),
		Error::MissingFunction("GetVersionNumber")
	);
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(query_version),
		..Default::default()
	};
	for raw_version in [sys::JNI_VERSION_1_8, sys::JVMTI_VERSION_1_1] {
		let mut context: Context = context(&raw const table, JvmtiVersion::V1_2);
		context.version = raw_version;
		// SAFETY: The mock returns an intentionally invalid or unsupported version.
		let error: Error = unsafe { Env::from_raw(&raw mut context.table) }.expect_err("reject version");
		assert!(matches!(
			error,
			Error::InvalidVersion(_) | Error::UnsupportedVersion { .. }
		));
	}
	assert_eq!(
		// SAFETY: Null is explicitly supported as an error case.
		unsafe { Env::from_raw_with_version(ptr::null_mut(), JvmtiVersion::V1_2) }.expect_err("null"),
		Error::NullPointer
	);
}

const unsafe extern "system" fn sampled_allocation(
	_env: *mut sys::jvmtiEnv,
	_jni: *mut sys::JNIEnv,
	_thread: sys::jthread,
	_object: sys::jobject,
	_class: sys::jclass,
	_size: sys::jlong,
) {
}

#[test]
fn sampling_and_early_capabilities_observe_their_own_version_boundaries() {
	let table = sys::jvmtiInterface_1_ {
		GetVersionNumber: Some(query_version),
		SetEventCallbacks: Some(register_callbacks),
		AddCapabilities: Some(reject_capabilities),
		..Default::default()
	};
	for version in [JvmtiVersion::V1_2, JvmtiVersion::V9, JvmtiVersion::V11] {
		let mut context: Context = context(&raw const table, version);
		// SAFETY: The initialized mock environment lives throughout this iteration.
		let env: Env<'_> = unsafe { Env::from_raw(&raw mut context.table) }.expect("environment");
		let callbacks = sys::jvmtiEventCallbacks {
			SampledObjectAlloc: Some(sampled_allocation),
			..Default::default()
		};
		// SAFETY: The mock accepts registration and the callback never unwinds.
		let registered: abos_jvmti::Result<()> = unsafe { env.set_event_callbacks(&callbacks) };
		if version < JvmtiVersion::V11 {
			assert_eq!(
				registered,
				Err(Error::UnsupportedVersion {
					required: JvmtiVersion::V11,
					actual: version
				})
			);
			assert_eq!(context.callback_bytes.get(), 0);
		} else {
			registered.expect("sampling callback");
		}
		let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		caps.set_can_generate_early_vmstart(true);
		// SAFETY: No reserved bits are set and the mock accepts this context.
		let acquired: abos_jvmti::Result<()> = unsafe { env.add_capabilities(&caps) };
		if version < JvmtiVersion::V9 {
			assert_eq!(
				acquired,
				Err(Error::UnsupportedVersion {
					required: JvmtiVersion::V9,
					actual: version
				})
			);
			assert_eq!(context.capability_calls.get(), 0);
		} else {
			assert_eq!(acquired, Err(Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE)));
		}
		assert_eq!(
			// SAFETY: A reserved event ID is rejected before reaching native code.
			unsafe { env.set_event_enabled(85, true) },
			Err(Error::Jvmti(sys::JVMTI_ERROR_INVALID_EVENT_TYPE)),
		);
	}
}
