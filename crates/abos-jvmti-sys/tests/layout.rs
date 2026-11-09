//! JDK-independent smoke checks for the raw declarations.
use core::{
	ffi::c_void,
	mem::{align_of, offset_of, size_of},
};

use abos_jvmti_sys as sys;

#[test]
fn tables_preserve_reserved_slots() {
	let slot: usize = size_of::<*mut c_void>();
	assert_eq!(size_of::<sys::jvmtiInterface_1_>(), 156 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1_, SetEventNotificationMode), slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1_, Allocate), 45 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1_, SetEventCallbacks), 121 * slot);
	assert_eq!(offset_of!(sys::jvmtiInterface_1_, SetHeapSamplingInterval), 155 * slot);
	assert_eq!(size_of::<sys::jvmtiEventCallbacks>(), 39 * slot);
	assert_eq!(offset_of!(sys::jvmtiEventCallbacks, VirtualThreadEnd), 38 * slot);
	assert_eq!(size_of::<sys::jvmtiCapabilities>(), 16);
	assert_eq!(align_of::<sys::jvmtiCapabilities>(), align_of::<u32>());
}

#[test]
fn capability_setters_preserve_neighbors_and_reserved_bits() {
	let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
	caps.set_can_tag_objects(true);
	caps.set_can_generate_garbage_collection_events(true);
	caps.set_can_generate_object_free_events(true);
	caps.set_can_support_virtual_threads(true);
	assert!(caps.can_tag_objects());
	assert!(caps.can_generate_garbage_collection_events());
	assert!(caps.can_generate_object_free_events());
	assert!(caps.can_support_virtual_threads());
	assert!(!caps.can_get_bytecodes());
	caps.set_can_tag_objects(false);
	caps.set_can_generate_garbage_collection_events(false);
	caps.set_can_generate_object_free_events(false);
	caps.set_can_support_virtual_threads(false);
	assert_eq!(caps, sys::jvmtiCapabilities::default());
}

#[test]
fn default_callback_table_disables_callbacks() {
	let callbacks: sys::jvmtiEventCallbacks = sys::jvmtiEventCallbacks::default();
	assert!(callbacks.VMInit.is_none());
	assert!(callbacks.ThreadStart.is_none());
	assert!(callbacks.reserved72.is_none());
	assert!(callbacks.VirtualThreadEnd.is_none());
	assert_eq!(sys::JVMTI_VERSION, 0x3019_0000);
}
