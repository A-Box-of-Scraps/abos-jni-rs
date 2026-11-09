//! JVM-backed instrumentation, debugging, and resource-lifecycle exercises.
use std::{
	ffi::c_void,
	ptr,
	sync::atomic::{AtomicUsize, Ordering},
};

use abos_jvmti::{ClassDefinition, Env, Error, JvmtiArray, JvmtiVersion, Result, sys};

#[path = "resources.rs"]
mod resources;

static BREAKPOINTS: AtomicUsize = AtomicUsize::new(0);
static STEPS: AtomicUsize = AtomicUsize::new(0);

pub fn request_capabilities(env: Env<'_>, caps: &mut sys::jvmtiCapabilities) {
	caps.set_can_retransform_classes(true);
	caps.set_can_redefine_classes(true);
	caps.set_can_get_bytecodes(true);
	caps.set_can_generate_breakpoint_events(true);
	caps.set_can_generate_single_step_events(true);
	caps.set_can_access_local_variables(true);
	caps.set_can_suspend(true);
	if env.version() >= JvmtiVersion::V21 {
		caps.set_can_support_virtual_threads(true);
	}
}

pub unsafe fn register(env: Env<'_>, jni: *mut sys::JNIEnv) -> Result<()> {
	ENVIRONMENT.store(env.as_raw(), Ordering::SeqCst);
	// SAFETY: VMInit provides a live attached JNI environment; references are local
	// to the callback, and registered functions remain in the loaded agent library.
	unsafe {
		let table: &sys::JNINativeInterface__1_6 = &*(*jni).cast::<sys::JNINativeInterface__1_6>();
		let class: sys::jclass = (table.FindClass)(jni, c"AgentFeatures".as_ptr());
		if (table.ExceptionCheck)(jni) != 0 || class.is_null() {
			return Err(Error::NullPointer);
		}
		let methods: [sys::JNINativeMethod; 3] = [
			sys::JNINativeMethod {
				name: c"configure".as_ptr().cast_mut(),
				signature: c"(Ljava/lang/Class;[B)V".as_ptr().cast_mut(),
				fnPtr: configure as *mut c_void,
			},
			sys::JNINativeMethod {
				name: c"exercise".as_ptr().cast_mut(),
				signature: c"(Ljava/lang/Thread;Ljava/lang/Object;Z)V".as_ptr().cast_mut(),
				fnPtr: exercise as *mut c_void,
			},
			sys::JNINativeMethod {
				name: c"completed".as_ptr().cast_mut(),
				signature: c"()V".as_ptr().cast_mut(),
				fnPtr: completed as *mut c_void,
			},
		];
		let status: sys::jint = (table.RegisterNatives)(jni, class, methods.as_ptr(), 3);
		let exception: bool = (table.ExceptionCheck)(jni) != 0;
		(table.DeleteLocalRef)(jni, class);
		if status != sys::JNI_OK || exception {
			return Err(Error::Jni(status));
		}
		env.set_event_enabled(sys::JVMTI_EVENT_BREAKPOINT, true)?;
	}
	Ok(())
}

unsafe fn environment() -> Result<Env<'static>> {
	// GetEnv creates a fresh connection, so use the existing agent environment.
	let raw: *mut sys::jvmtiEnv = ENVIRONMENT.load(Ordering::SeqCst);
	// SAFETY: The agent retains this environment until VM termination. Callers use
	// the handle only within a native call, before VMDeath.
	unsafe { Env::from_raw(raw) }
}

static ENVIRONMENT: std::sync::atomic::AtomicPtr<sys::jvmtiEnv> = std::sync::atomic::AtomicPtr::new(ptr::null_mut());

unsafe extern "system" fn configure(
	jni: *mut sys::JNIEnv,
	_class: sys::jclass,
	target: sys::jclass,
	bytes: sys::jbyteArray,
) {
	super::callback(|| {
		// SAFETY: JNI native arguments remain valid for this call in the live phase.
		unsafe {
			let env: Env<'_> = environment()?;
			let table: &sys::JNINativeInterface__1_6 = &*(*jni).cast::<sys::JNINativeInterface__1_6>();
			let method: sys::jmethodID =
				(table.GetMethodID)(jni, target, c"compute".as_ptr(), c"(IJFDLjava/lang/Object;)J".as_ptr());
			if (table.ExceptionCheck)(jni) != 0 || method.is_null() {
				return Err(Error::NullPointer);
			}
			let code: JvmtiArray<'_, '_, u8> = env.bytecodes(method)?;
			assert!(!code.is_empty());
			code.free()?;
			let count: sys::jsize = (table.GetArrayLength)(jni, bytes);
			let mut replacement: Vec<u8> = vec![0; usize::try_from(count).expect("nonnegative length")];
			(table.GetByteArrayRegion)(jni, bytes, 0, count, replacement.as_mut_ptr().cast());
			if (table.ExceptionCheck)(jni) != 0 {
				return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
			}
			env.retransform_classes(&[target])?;
			env.redefine_classes(&[ClassDefinition {
				class: target,
				bytes: &replacement,
			}])?;
			assert!(matches!(
				env.redefine_classes(&[ClassDefinition {
					class: target,
					bytes: &[0; 8]
				}]),
				Err(Error::Jvmti(sys::JVMTI_ERROR_INVALID_CLASS_FORMAT))
			));
			env.set_breakpoint(method, 0)?;
			assert_eq!(
				env.set_breakpoint(method, 0),
				Err(Error::Jvmti(sys::JVMTI_ERROR_DUPLICATE))
			);
			super::report(format_args!("JVMTI_INSTRUMENTATION_OK"));
		}
		Ok(())
	});
}

pub unsafe extern "system" fn breakpoint(
	raw: *mut sys::jvmtiEnv,
	jni: *mut sys::JNIEnv,
	thread: sys::jthread,
	method: sys::jmethodID,
	location: sys::jlocation,
) {
	super::callback(|| {
		// SAFETY: The event runs on the current Java thread at the target method's
		// entry. Its parameters occupy slots 1,2,4,5,7, with receiver in slot 0.
		unsafe {
			let env: Env<'_> = Env::from_raw(raw)?;
			assert_eq!(env.get_local_int(thread, 0, 1)?, 7);
			assert_eq!(env.get_local_long(thread, 0, 2)?, 8);
			assert_eq!(env.get_local_float(thread, 0, 4)?.to_bits(), 3.5_f32.to_bits());
			assert_eq!(env.get_local_double(thread, 0, 5)?.to_bits(), 4.5_f64.to_bits());
			let object: sys::jobject = env.get_local_object(thread, 0, 7)?;
			assert!(!object.is_null());
			let receiver: sys::jobject = env.local_instance(thread, 0)?;
			let slot_zero: sys::jobject = env.get_local_object(thread, 0, 0)?;
			let table: &sys::JNINativeInterface__1_6 = &*(*jni).cast::<sys::JNINativeInterface__1_6>();
			assert_ne!((table.IsSameObject)(jni, receiver, slot_zero), 0);
			for reference in [object, receiver, slot_zero] {
				(table.DeleteLocalRef)(jni, reference);
			}
			env.set_local_int(thread, 0, 1, 42)?;
			env.set_local_long(thread, 0, 2, 84)?;
			env.set_local_float(thread, 0, 4, 1.5)?;
			env.set_local_double(thread, 0, 5, 2.5)?;
			env.set_local_object(thread, 0, 7, ptr::null_mut())?;
			assert_eq!(
				env.get_local_int(thread, 0, 999),
				Err(Error::Jvmti(sys::JVMTI_ERROR_INVALID_SLOT))
			);
			let frames: Vec<sys::jvmtiFrameInfo> = env.stack_trace(thread, 0, 4)?;
			assert_eq!(frames[0].method, method);
			env.clear_breakpoint(method, location)?;
			assert_eq!(
				env.clear_breakpoint(method, location),
				Err(Error::Jvmti(sys::JVMTI_ERROR_NOT_FOUND))
			);
			assert_eq!(
				env.set_thread_event_enabled(thread, sys::JVMTI_EVENT_VM_DEATH, true),
				Err(Error::Jvmti(sys::JVMTI_ERROR_ILLEGAL_ARGUMENT))
			);
			env.set_single_step_enabled(thread, true)?;
			BREAKPOINTS.fetch_add(1, Ordering::SeqCst);
		}
		Ok(())
	});
}

pub unsafe extern "system" fn single_step(
	raw: *mut sys::jvmtiEnv,
	_jni: *mut sys::JNIEnv,
	thread: sys::jthread,
	_method: sys::jmethodID,
	_location: sys::jlocation,
) {
	super::callback(|| {
		// SAFETY: Event callback on the current thread, in live phase.
		unsafe {
			Env::from_raw(raw)?.set_single_step_enabled(thread, false)?;
		}
		STEPS.fetch_add(1, Ordering::SeqCst);
		Ok(())
	});
}

unsafe extern "system" fn exercise(
	_jni: *mut sys::JNIEnv,
	_class: sys::jclass,
	worker: sys::jthread,
	root: sys::jobject,
	virtual_thread: sys::jboolean,
) {
	super::callback(|| {
		// SAFETY: Native arguments are live JNI references. Java keeps the worker
		// alive, not holding locks required here, until after this call returns.
		unsafe {
			let env: Env<'_> = environment()?;
			resources::threads(env, worker, virtual_thread != 0)?;
			resources::heap(env, root)?;
			resources::monitor(env)?;
			resources::management(env, root)?;
		}
		Ok(())
	});
}

unsafe extern "system" fn completed(_jni: *mut sys::JNIEnv, _class: sys::jclass) {
	super::callback(|| {
		assert_eq!(BREAKPOINTS.load(Ordering::SeqCst), 1);
		assert_eq!(STEPS.load(Ordering::SeqCst), 1);
		// SAFETY: Called in live phase after the only instrumented invocation.
		unsafe {
			environment()?.set_event_enabled(sys::JVMTI_EVENT_BREAKPOINT, false)?;
		}
		super::report(format_args!("JVMTI_DEBUGGING_OK"));
		Ok(())
	});
}
