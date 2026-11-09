use std::{ffi::c_void, ptr};

use abos_jni::jni_str;
use abos_jvmti::{Env, Error, JvmtiVersion, RawMonitor, RawMonitorGuard, Result, sys};

pub(super) unsafe fn threads(env: Env<'_>, worker: sys::jthread, virtual_thread: bool) -> Result<()> {
	// SAFETY: Java keeps this worker alive, without holding locks needed here.
	// Resume is attempted even if inspecting the suspended thread fails.
	unsafe {
		env.suspend_thread(worker)?;
		let duplicate: Result<()> = env.suspend_thread(worker);
		let frames: Result<Vec<sys::jvmtiFrameInfo>> = env.stack_trace(worker, 0, 16);
		let empty: Result<Vec<sys::jvmtiFrameInfo>> = env.stack_trace(worker, 0, 0);
		env.resume_thread(worker)?;
		// Assert only after resumption so even assertion failures cannot strand the worker.
		assert_eq!(duplicate, Err(Error::Jvmti(sys::JVMTI_ERROR_THREAD_SUSPENDED)));
		assert!(!frames?.is_empty());
		assert!(empty?.is_empty());
		assert_eq!(
			env.resume_thread(worker),
			Err(Error::Jvmti(sys::JVMTI_ERROR_THREAD_NOT_SUSPENDED))
		);
		if virtual_thread {
			assert!(env.version() >= JvmtiVersion::V21);
			// Excluded threads remain runnable, then test bulk suspension and resumption.
			env.suspend_all_virtual_threads(&[worker])?;
			let excluded: Result<()> = env.resume_thread(worker);
			env.resume_all_virtual_threads(&[])?;
			assert_eq!(excluded, Err(Error::Jvmti(sys::JVMTI_ERROR_THREAD_NOT_SUSPENDED)));
			env.suspend_all_virtual_threads(&[])?;
			let duplicate: Result<()> = env.suspend_thread(worker);
			let frames: Result<Vec<sys::jvmtiFrameInfo>> = env.stack_trace(worker, 0, 16);
			env.resume_all_virtual_threads(&[])?;
			assert_eq!(duplicate, Err(Error::Jvmti(sys::JVMTI_ERROR_THREAD_SUSPENDED)));
			assert!(!frames?.is_empty());
			super::super::report(format_args!("JVMTI_VIRTUAL_THREADS_OK"));
		}
	}
	super::super::report(format_args!("JVMTI_THREADS_OK"));
	Ok(())
}

unsafe extern "system" fn visit_object(
	_class_tag: sys::jlong,
	_size: sys::jlong,
	tag: *mut sys::jlong,
	_length: sys::jint,
	data: *mut c_void,
) -> sys::jint {
	// SAFETY: Synchronous heap traversal supplies a tag slot; data is our live counter.
	unsafe {
		if *tag == 12345 {
			*data.cast::<usize>() += 1;
			return sys::JVMTI_VISIT_ABORT;
		}
	}
	0
}

unsafe extern "system" fn visit_reference(
	_kind: sys::jvmtiHeapReferenceKind,
	_info: *const sys::jvmtiHeapReferenceInfo,
	_class_tag: sys::jlong,
	_referrer_class_tag: sys::jlong,
	_size: sys::jlong,
	_tag: *mut sys::jlong,
	_referrer_tag: *mut sys::jlong,
	_length: sys::jint,
	data: *mut c_void,
) -> sys::jint {
	// SAFETY: Synchronous traversal supplies the live counter, with no concurrent access.
	unsafe {
		*data.cast::<usize>() += 1;
	}
	sys::JVMTI_VISIT_ABORT
}

pub(super) unsafe fn heap(env: Env<'_>, root: sys::jobject) -> Result<()> {
	// SAFETY: Root is a local JNI object-array reference in this native call.
	// Callbacks only read tags/update stack counters and never invoke JNI or panic.
	unsafe {
		assert_eq!(env.tag(root)?, 0);
		env.set_tag(root, 12345)?;
		let traversal: Result<()> = (|| {
			assert_eq!(env.tag(root)?, 12345);
			let mut objects: usize = 0;
			let mut references: usize = 0;
			let callbacks = sys::jvmtiHeapCallbacks {
				heap_iteration_callback: Some(visit_object),
				heap_reference_callback: Some(visit_reference),
				..Default::default()
			};
			env.iterate_through_heap(0, ptr::null_mut(), &callbacks, (&raw mut objects).cast())?;
			env.follow_references(0, ptr::null_mut(), root, &callbacks, (&raw mut references).cast())?;
			assert_eq!(objects, 1);
			assert!(references > 0);
			Ok(())
		})();
		let cleared: Result<()> = env.set_tag(root, 0);
		traversal.and(cleared)?;
		assert_eq!(env.tag(root)?, 0);
	}
	super::super::report(format_args!("JVMTI_HEAP_OK"));
	Ok(())
}

pub(super) unsafe fn monitor(env: Env<'_>) -> Result<()> {
	// SAFETY: Live attached thread; this function is the monitor's only user and
	// releases every entry before destruction, with bounded waits.
	unsafe {
		let monitor: RawMonitor<'_, '_> = env.create_raw_monitor(jni_str!("abos-feature-monitor"))?;
		let guard: RawMonitorGuard<'_, '_, '_> = monitor.enter()?;
		{
			let _recursive: RawMonitorGuard<'_, '_, '_> = monitor.enter()?;
		}
		guard.notify()?;
		guard.notify_all()?;
		assert_eq!(guard.wait(1), Err(Error::Jvmti(sys::JVMTI_ERROR_INTERRUPT)));
		guard.wait(1)?;
		guard.unlock()?;
		monitor.destroy()?;
		// Exercise ownership transfer/adoption and best-effort guard/monitor cleanup.
		let raw: sys::jrawMonitorID = env.create_raw_monitor(jni_str!("abos-adopted-monitor"))?.into_raw();
		let adopted: RawMonitor<'_, '_> = RawMonitor::from_raw(&env, raw)?;
		drop(adopted.enter()?);
		drop(adopted);
	}
	super::super::report(format_args!("JVMTI_MONITORS_OK"));
	Ok(())
}

pub(super) unsafe fn management(env: Env<'_>, root: sys::jobject) -> Result<()> {
	// SAFETY: No other callback accesses the storage slot; clear it before the
	// stack value goes out of scope. Tag capability changes are serialized by Java.
	unsafe {
		assert!(env.environment_local_storage()?.is_null());
		let mut value: usize = 42;
		let pointer: *mut c_void = (&raw mut value).cast();
		env.set_environment_local_storage(pointer)?;
		let stored: Result<*mut c_void> = env.environment_local_storage();
		let cleared: Result<()> = env.set_environment_local_storage(ptr::null_mut());
		assert_eq!(stored?, pointer);
		cleared?;
		assert!(env.environment_local_storage()?.is_null());
		let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		caps.set_can_tag_objects(true);
		env.relinquish_capabilities(&caps)?;
		assert!(!env.capabilities()?.can_tag_objects());
		let unavailable: Result<sys::jlong> = env.tag(root);
		let callbacks: sys::jvmtiHeapCallbacks = sys::jvmtiHeapCallbacks::default();
		let traversal: Result<()> = env.iterate_through_heap(0, ptr::null_mut(), &callbacks, ptr::null_mut());
		// Tagging is available in live phase, unlike many debugging capabilities.
		env.add_capabilities(&caps)?;
		assert_eq!(unavailable, Err(Error::Jvmti(sys::JVMTI_ERROR_MUST_POSSESS_CAPABILITY)));
		assert_eq!(traversal, Err(Error::Jvmti(sys::JVMTI_ERROR_MUST_POSSESS_CAPABILITY)));
	}
	super::super::report(format_args!("JVMTI_MANAGEMENT_OK"));
	Ok(())
}
