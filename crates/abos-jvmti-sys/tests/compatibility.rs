//! Historical views retain ABI slots without requiring the latest table length.
use core::mem::{offset_of, size_of};

use abos_jvmti_sys as sys;

#[test]
fn historical_tables_preserve_holes_and_end_at_the_last_available_slot() {
	let slot: usize = size_of::<*const ()>();
	assert_eq!(size_of::<sys::jvmtiInterface_1__1_2>(), 155 * slot);
	assert_eq!(size_of::<sys::jvmtiInterface_1__9>(), 155 * slot);
	assert_eq!(size_of::<sys::jvmtiInterface_1__11>(), 156 * slot);
	assert_eq!(size_of::<sys::jvmtiInterface_1__21>(), 156 * slot);
	assert_eq!(
		size_of::<sys::jvmtiInterface_1__25>(),
		size_of::<sys::jvmtiInterface_1_>()
	);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__1_2, reserved3), 2 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__9, GetAllModules), 2 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__1_2, reserved67), 66 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__21, reserved67), 66 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__25, ClearAllFramePops), 66 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1__11, reserved118), 117 * slot);
	assert_eq!(
		offset_of!(sys::jvmtiInterface_1__21, SuspendAllVirtualThreads),
		117 * slot
	);
	assert_eq!(
		offset_of!(sys::jvmtiInterface_1__11, SetHeapSamplingInterval),
		155 * slot
	);
}

#[test]
fn event_and_capability_introduction_versions_are_independent() {
	assert_eq!(
		sys::jvmti_event_version(sys::JVMTI_EVENT_VM_INIT),
		Some(sys::JVMTI_VERSION_1_2)
	);
	assert_eq!(
		sys::jvmti_event_version(sys::JVMTI_EVENT_SAMPLED_OBJECT_ALLOC),
		Some(sys::JVMTI_VERSION_11)
	);
	assert_eq!(
		sys::jvmti_event_version(sys::JVMTI_EVENT_VIRTUAL_THREAD_END),
		Some(sys::JVMTI_VERSION_21)
	);
	for event in [0, 49, 72, 77, 78, 79, 85, 89, u32::MAX] {
		assert_eq!(sys::jvmti_event_version(event), None);
	}
	let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
	assert_eq!(caps.required_version(), sys::JVMTI_VERSION_1_2);
	caps.set_can_generate_early_vmstart(true);
	assert_eq!(caps.required_version(), sys::JVMTI_VERSION_9);
	caps.set_can_generate_early_class_hook_events(true);
	assert_eq!(caps.required_version(), sys::JVMTI_VERSION_9);
	caps.set_can_generate_sampled_object_alloc_events(true);
	assert_eq!(caps.required_version(), sys::JVMTI_VERSION_11);
	caps.set_can_support_virtual_threads(true);
	assert_eq!(caps.required_version(), sys::JVMTI_VERSION_21);
}
