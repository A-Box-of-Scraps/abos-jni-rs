//! Failure handling and version boundaries for feature wrappers.
#![allow(unsafe_code, reason = "Mock tables validate FFI arguments and output handling.")]

use std::{cell::Cell, ptr};

use abos_jvmti::{ClassDefinition, Env, Error, JvmtiVersion, Result, sys};

#[repr(C)]
struct Context {
	table: sys::jvmtiEnv,
	count: Cell<sys::jint>,
	error: Cell<sys::jvmtiError>,
	calls: Cell<usize>,
}

fn with_table(table: &sys::jvmtiInterface_1_, test: impl FnOnce(&Context, Env<'_>)) {
	let mut context = Context {
		table,
		count: Cell::new(1),
		error: Cell::new(sys::JVMTI_ERROR_NONE),
		calls: Cell::new(0),
	};
	// SAFETY: The initialized context and table outlive all operations in the closure.
	let env: Env<'_> = unsafe { Env::from_raw_with_version(&raw mut context.table, JvmtiVersion::V21) }.expect("env");
	test(&context, env);
}

unsafe extern "system" fn stack_trace(
	raw: *mut sys::jvmtiEnv,
	thread: sys::jthread,
	start: sys::jint,
	maximum: sys::jint,
	frames: *mut sys::jvmtiFrameInfo,
	count: *mut sys::jint,
) -> sys::jvmtiError {
	// SAFETY: repr(C) context and output pointers are supplied by the wrapper.
	unsafe {
		let context: &Context = &*raw.cast::<Context>();
		context.calls.set(context.calls.get() + 1);
		assert_eq!(thread, ptr::dangling_mut());
		assert_eq!(start, -1);
		assert!(!frames.is_null());
		if maximum > 0 {
			*frames = sys::jvmtiFrameInfo {
				method: ptr::dangling_mut(),
				location: 19,
			};
		}
		*count = context.count.get();
		context.error.get()
	}
}

#[test]
fn stack_buffers_validate_counts_and_ignore_undefined_error_outputs() {
	let table = sys::jvmtiInterface_1_ {
		GetStackTrace: Some(stack_trace),
		..Default::default()
	};
	with_table(&table, |context, env| {
		// SAFETY: Mock validates buffer sizes and never dereferences the sentinel thread.
		unsafe {
			let frames: Vec<sys::jvmtiFrameInfo> = env.stack_trace(ptr::dangling_mut(), -1, 2).expect("trace");
			assert_eq!(frames.len(), 1);
			assert_eq!(frames[0].location, 19);
			context.count.set(0);
			assert!(
				env.stack_trace(ptr::dangling_mut(), -1, 0)
					.expect("empty trace")
					.is_empty()
			);
			for count in [-1, 3] {
				context.count.set(count);
				assert_eq!(
					env.stack_trace(ptr::dangling_mut(), -1, 2).expect_err("bad count"),
					Error::InvalidCount(count)
				);
			}
			context.error.set(sys::JVMTI_ERROR_THREAD_NOT_ALIVE);
			assert_eq!(
				env.stack_trace(ptr::dangling_mut(), -1, 2).expect_err("native error"),
				Error::Jvmti(sys::JVMTI_ERROR_THREAD_NOT_ALIVE)
			);
			let calls: usize = context.calls.get();
			assert_eq!(
				env.stack_trace(ptr::dangling_mut(), -1, usize::MAX).expect_err("size"),
				Error::AllocationTooLarge
			);
			assert_eq!(context.calls.get(), calls);
		}
	});
}

unsafe extern "system" fn redefine(
	raw: *mut sys::jvmtiEnv,
	count: sys::jint,
	definitions: *const sys::jvmtiClassDefinition,
) -> sys::jvmtiError {
	// SAFETY: Wrapper passes initialized descriptors and borrowed class bytes.
	unsafe {
		let context: &Context = &*raw.cast::<Context>();
		context.calls.set(context.calls.get() + 1);
		assert_eq!(count, context.count.get());
		if count == 1 {
			assert_eq!((*definitions).klass, ptr::dangling_mut());
			assert_eq!((*definitions).class_byte_count, 4);
			assert_eq!(
				std::slice::from_raw_parts((*definitions).class_bytes, 4),
				[0xca, 0xfe, 0xba, 0xbe]
			);
		}
		context.error.get()
	}
}

unsafe extern "system" fn retransform(
	raw: *mut sys::jvmtiEnv,
	count: sys::jint,
	classes: *const sys::jclass,
) -> sys::jvmtiError {
	// SAFETY: Wrapper passes a readable class-reference slice; no reference is dereferenced.
	unsafe {
		let context: &Context = &*raw.cast::<Context>();
		context.calls.set(context.calls.get() + 1);
		assert_eq!(count, context.count.get());
		if count == 1 {
			assert_eq!(*classes, ptr::dangling_mut());
		}
		context.error.get()
	}
}

#[test]
fn instrumentation_borrows_inputs_handles_empty_lists_and_preserves_errors() {
	let table = sys::jvmtiInterface_1_ {
		RedefineClasses: Some(redefine),
		RetransformClasses: Some(retransform),
		..Default::default()
	};
	with_table(&table, |context, env| {
		let bytes: [u8; 4] = [0xca, 0xfe, 0xba, 0xbe];
		let definitions: [ClassDefinition<'_>; 1] = [ClassDefinition {
			class: ptr::dangling_mut(),
			bytes: &bytes,
		}];
		for error in [sys::JVMTI_ERROR_NONE, sys::JVMTI_ERROR_UNMODIFIABLE_CLASS] {
			context.error.set(error);
			let expected: Result<()> = if error == sys::JVMTI_ERROR_NONE {
				Ok(())
			} else {
				Err(Error::Jvmti(error))
			};
			// SAFETY: Mock ignores sentinel JNI handles and borrows only initialized data.
			unsafe {
				context.count.set(1);
				assert_eq!(env.redefine_classes(&definitions), expected);
				assert_eq!(env.retransform_classes(&[ptr::dangling_mut()]), expected);
				context.count.set(0);
				assert_eq!(env.redefine_classes(&[]), expected);
				assert_eq!(env.retransform_classes(&[]), expected);
			}
		}
		assert_eq!(context.calls.get(), 8);
	});
}

#[test]
fn virtual_operations_reject_baseline_reserved_slots_before_access() {
	let table: Box<sys::jvmtiInterface_1__1_2> = Box::default();
	let mut raw: sys::jvmtiEnv = (&raw const *table).cast();
	// SAFETY: Only a baseline table is allocated; version gates must precede any newer borrow.
	let env: Env<'_> = unsafe { Env::from_raw_with_version(&raw mut raw, JvmtiVersion::V1_2) }.expect("baseline");
	let error: Error = Error::UnsupportedVersion {
		required: JvmtiVersion::V21,
		actual: JvmtiVersion::V1_2,
	};
	// SAFETY: Empty lists have no handles; all operations are rejected by version checks.
	unsafe {
		assert_eq!(env.suspend_all_virtual_threads(&[]), Err(error));
		assert_eq!(env.resume_all_virtual_threads(&[]), Err(error));
		let mut caps: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		caps.set_can_support_virtual_threads(true);
		assert_eq!(env.relinquish_capabilities(&caps), Err(error));
		assert_eq!(
			env.set_thread_event_enabled(ptr::null_mut(), sys::JVMTI_EVENT_VIRTUAL_THREAD_START, true),
			Err(error)
		);
	}
}

unsafe extern "system" fn virtual_threads(
	raw: *mut sys::jvmtiEnv,
	count: sys::jint,
	exceptions: *const sys::jthread,
) -> sys::jvmtiError {
	// SAFETY: Wrapper supplies a readable exception slice and the live mock context.
	unsafe {
		let context: &Context = &*raw.cast::<Context>();
		context.calls.set(context.calls.get() + 1);
		assert_eq!(count, context.count.get());
		if count == 1 {
			assert_eq!(*exceptions, ptr::dangling_mut());
		}
		context.error.get()
	}
}

#[test]
fn virtual_operations_forward_exceptions_and_vm_errors() {
	let table = sys::jvmtiInterface_1_ {
		SuspendAllVirtualThreads: Some(virtual_threads),
		ResumeAllVirtualThreads: Some(virtual_threads),
		..Default::default()
	};
	with_table(&table, |context, env| {
		// SAFETY: Mock reads only the borrowed array, not the sentinel JNI references.
		unsafe {
			env.suspend_all_virtual_threads(&[ptr::dangling_mut()])
				.expect("suspend");
			context.count.set(0);
			env.resume_all_virtual_threads(&[]).expect("resume");
			context.error.set(sys::JVMTI_ERROR_MUST_POSSESS_CAPABILITY);
			assert_eq!(
				env.suspend_all_virtual_threads(&[]),
				Err(Error::Jvmti(sys::JVMTI_ERROR_MUST_POSSESS_CAPABILITY))
			);
			assert_eq!(
				env.resume_all_virtual_threads(&[]),
				Err(Error::Jvmti(sys::JVMTI_ERROR_MUST_POSSESS_CAPABILITY))
			);
		}
		assert_eq!(context.calls.get(), 4);
	});
}

#[test]
fn feature_groups_report_missing_functions_without_native_calls() {
	with_table(&sys::jvmtiInterface_1_::default(), |_context, env| {
		// SAFETY: No native function is installed; each wrapper must reject before using handles.
		unsafe {
			assert_eq!(
				env.bytecodes(ptr::null_mut()).expect_err("missing bytecodes"),
				Error::MissingFunction("GetBytecodes")
			);
			assert_eq!(
				env.set_breakpoint(ptr::null_mut(), 0),
				Err(Error::MissingFunction("SetBreakpoint"))
			);
			assert_eq!(
				env.clear_breakpoint(ptr::null_mut(), 0),
				Err(Error::MissingFunction("ClearBreakpoint"))
			);
			assert_eq!(
				env.get_local_int(ptr::null_mut(), 0, 0),
				Err(Error::MissingFunction("GetLocalInt"))
			);
			assert_eq!(
				env.set_local_object(ptr::null_mut(), 0, 0, ptr::null_mut()),
				Err(Error::MissingFunction("SetLocalObject"))
			);
			assert_eq!(
				env.suspend_thread(ptr::null_mut()),
				Err(Error::MissingFunction("SuspendThread"))
			);
			assert_eq!(
				env.resume_thread(ptr::null_mut()),
				Err(Error::MissingFunction("ResumeThread"))
			);
			assert_eq!(
				env.suspend_all_virtual_threads(&[]),
				Err(Error::MissingFunction("SuspendAllVirtualThreads"))
			);
			assert_eq!(
				env.resume_all_virtual_threads(&[]),
				Err(Error::MissingFunction("ResumeAllVirtualThreads"))
			);
		}
	});
}

#[test]
fn heap_and_management_report_missing_functions_without_native_calls() {
	with_table(&sys::jvmtiInterface_1_::default(), |_context, env| {
		// SAFETY: No native functions are installed; all handles are rejected without use.
		unsafe {
			assert_eq!(env.tag(ptr::null_mut()), Err(Error::MissingFunction("GetTag")));
			assert_eq!(env.set_tag(ptr::null_mut(), 1), Err(Error::MissingFunction("SetTag")));
			let callbacks: sys::jvmtiHeapCallbacks = sys::jvmtiHeapCallbacks::default();
			assert_eq!(
				env.iterate_through_heap(0, ptr::null_mut(), &callbacks, ptr::null_mut()),
				Err(Error::MissingFunction("IterateThroughHeap"))
			);
			assert_eq!(
				env.follow_references(0, ptr::null_mut(), ptr::null_mut(), &callbacks, ptr::null_mut()),
				Err(Error::MissingFunction("FollowReferences"))
			);
			assert_eq!(
				env.environment_local_storage(),
				Err(Error::MissingFunction("GetEnvironmentLocalStorage"))
			);
			assert_eq!(
				env.set_environment_local_storage(ptr::null_mut()),
				Err(Error::MissingFunction("SetEnvironmentLocalStorage"))
			);
			assert_eq!(
				env.set_single_step_enabled(ptr::null_mut(), true),
				Err(Error::MissingFunction("SetEventNotificationMode"))
			);
			assert_eq!(
				env.relinquish_capabilities(&sys::jvmtiCapabilities::default()),
				Err(Error::MissingFunction("RelinquishCapabilities"))
			);
		}
	});
}
