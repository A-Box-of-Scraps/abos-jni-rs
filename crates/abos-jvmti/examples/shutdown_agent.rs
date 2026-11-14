//! Explicit live-phase shutdown, without claiming that event disabling synchronizes callbacks.
//!
//! Run with tests/java/ShutdownTarget.java; see the crate README for build commands.
//! The library, gate, and native entry points stay loaded for the entire VM lifetime,
//! including after disposal. This example is not a dynamic library unload protocol.
#![allow(
	unsafe_code,
	reason = "Native agent callbacks and explicitly coordinated JVMTI resource ownership."
)]

use std::{
	ffi::{c_char, c_void},
	io::Write,
	ptr,
	sync::{
		Mutex, MutexGuard,
		atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
	},
};

use abos_jvmti::{Env, Error, JvmtiArray, Result, catch_callback, catch_callback_with, sys};

#[path = "shutdown_agent/protocol.rs"]
mod protocol;
use protocol::{Gate, Operations, Progress, Shutdown};

// This gate cannot live in environment-local storage: a late callback must check it
// without touching an environment that might already have been disposed.
static GATE: Gate = Gate::new();
static FAILED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static BUFFER: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static AGENT: Mutex<Option<Agent>> = Mutex::new(None);

struct Agent {
	env: Option<Env<'static>>,
	shutdown: Shutdown<'static>,
}

fn failed() {
	FAILED.store(true, Ordering::Release);
	GATE.close();
}

unsafe extern "system" fn thread_start(raw: *mut sys::jvmtiEnv, _jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	// Gate entry/drop cannot panic. Keep the admission through panic handling so
	// failure flags are published before the coordinator can observe a drained gate.
	let Some(_admission) = GATE.enter() else { return };
	catch_callback_with(
		(),
		|| {
			// SAFETY: ThreadStart is delivered on an attached thread in live phase. The
			// admission prevents cleanup/disposal while this callback uses the buffer/env.
			let result: Result<()> = unsafe {
				(|| {
					let env: Env<'_> = Env::from_raw(raw)?;
					let buffer: *mut u8 = BUFFER.load(Ordering::Acquire);
					if buffer.is_null() || buffer.read() != 42 || env.phase()? != sys::JVMTI_PHASE_LIVE {
						return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
					}
					CALLS.fetch_add(1, Ordering::Relaxed);
					Ok(())
				})()
			};
			if result.is_err() {
				failed();
			}
		},
		failed,
	);
}

unsafe extern "system" fn vm_init(raw: *mut sys::jvmtiEnv, jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	// Gate entry/drop cannot panic. Keep the admission through panic handling so
	// failure flags are published before the coordinator can observe a drained gate.
	let Some(_admission) = GATE.enter() else { return };
	catch_callback_with(
		(),
		|| {
			// SAFETY: VMInit provides attached JNI/JVMTI environments in live phase.
			// Setup completes before Java main; no later callback writes to the buffer.
			let result: Result<()> = unsafe {
				(|| {
					let env: Env<'_> = Env::from_raw(raw)?;
					let mut buffer: abos_jvmti::Allocation<'_, '_> = env.allocate(1)?;
					buffer[0] = 42;
					BUFFER.store(buffer.into_raw(), Ordering::Release);
					register(jni)?;
					env.set_event_enabled(sys::JVMTI_EVENT_THREAD_START, true)
				})()
			};
			if result.is_err() {
				failed();
			}
		},
		failed,
	);
}

unsafe fn register(jni: *mut sys::JNIEnv) -> Result<()> {
	// SAFETY: Called only from VMInit; local references are confined to this call.
	unsafe {
		let table: &sys::JNINativeInterface__1_6 = &*(*jni).cast::<sys::JNINativeInterface__1_6>();
		let class: sys::jclass = (table.FindClass)(jni, c"ShutdownTarget".as_ptr());
		if class.is_null() || (table.ExceptionCheck)(jni) != 0 {
			return Err(Error::NullPointer);
		}
		let methods: [sys::JNINativeMethod; 2] = [
			sys::JNINativeMethod {
				name: c"pollShutdown".as_ptr().cast_mut(),
				signature: c"()I".as_ptr().cast_mut(),
				fnPtr: poll_shutdown as *mut c_void,
			},
			sys::JNINativeMethod {
				name: c"callbackCount".as_ptr().cast_mut(),
				signature: c"()J".as_ptr().cast_mut(),
				fnPtr: callback_count as *mut c_void,
			},
		];
		let status: sys::jint = (table.RegisterNatives)(jni, class, methods.as_ptr(), 2);
		(table.DeleteLocalRef)(jni, class);
		if status != sys::JNI_OK || (table.ExceptionCheck)(jni) != 0 {
			return Err(Error::Jni(status));
		}
		Ok(())
	}
}

struct NativeOperations<'vm> {
	env: Env<'vm>,
}

impl Operations for NativeOperations<'_> {
	fn disable_delivery(&mut self) -> Result<()> {
		// SAFETY: Only the serialized native poller constructs these operations, in
		// live phase on an attached thread. No per-thread event enables exist here.
		unsafe {
			self.env.set_event_enabled(sys::JVMTI_EVENT_THREAD_START, false)?;
			self.env.set_event_enabled(sys::JVMTI_EVENT_VM_INIT, false)?;
			self.env.set_event_callbacks(&sys::jvmtiEventCallbacks::default())
		}
	}

	fn release_resources(&mut self) -> Result<()> {
		let buffer: *mut u8 = BUFFER.swap(ptr::null_mut(), Ordering::AcqRel);
		if !buffer.is_null() {
			// SAFETY: Admission is closed and drained. We exclusively recover the
			// initialized allocation from this environment on an attached coordinator.
			unsafe {
				JvmtiArray::from_raw(&self.env, buffer, 1)?.free()?;
			}
		}
		Ok(())
	}

	fn dispose(&mut self) -> Result<()> {
		// SAFETY: All admitted environment users have finished, the buffer is freed,
		// and this agent owns no workers, monitors, or suspended threads. Late callback
		// entries consult only the static closed gate; no environment lookup is allowed.
		unsafe { self.env.dispose() }
	}
}

unsafe extern "system" fn poll_shutdown(_jni: *mut sys::JNIEnv, _class: sys::jclass) -> sys::jint {
	catch_callback(sys::JNI_ERR, || {
		let mut slot: MutexGuard<'_, Option<Agent>> = if let Ok(slot) = AGENT.lock() {
			slot
		} else {
			failed();
			return sys::JNI_ERR;
		};
		let Some(agent) = slot.as_mut() else {
			return sys::JNI_ERR;
		};
		let Some(env) = agent.env else {
			return if FAILED.load(Ordering::Acquire) {
				sys::JNI_ERR
			} else {
				1
			};
		};
		let mut operations: NativeOperations<'_> = NativeOperations { env };
		let progress: Result<Progress> = agent.shutdown.poll(&mut operations);
		if progress == Ok(Progress::Complete) {
			agent.env = None;
		}
		drop(slot);
		match progress {
			Ok(Progress::Pending) => 0,
			Ok(Progress::Complete) => {
				if FAILED.load(Ordering::Acquire) {
					return sys::JNI_ERR;
				}
				let _output: std::io::Result<()> = writeln!(std::io::stderr().lock(), "JVMTI_SHUTDOWN_OK");
				1
			}
			Err(_) => {
				failed();
				sys::JNI_ERR
			}
		}
	})
}

unsafe extern "system" fn callback_count(_jni: *mut sys::JNIEnv, _class: sys::jclass) -> sys::jlong {
	// No environment access, including after disposal. A bounded conversion cannot unwind.
	i64::try_from(CALLS.load(Ordering::Relaxed)).unwrap_or(i64::MAX)
}

/// Starts the example agent. Failed setup leaves state pinned for VM teardown.
///
/// # Safety
/// The VM must supply a live `OnLoad` VM pointer and keep this library loaded until
/// VM termination, including after the example reports successful disposal.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Agent_OnLoad(
	vm: *mut sys::JavaVM,
	_options: *mut c_char,
	_reserved: *mut c_void,
) -> sys::jint {
	catch_callback_with(
		sys::JNI_ERR,
		|| {
			// SAFETY: OnLoad permits construction and setup. This handle is used only
			// behind the gate or by the serialized coordinator, and cleared on disposal.
			let result: Result<()> = unsafe {
				(|| {
					let env: Env<'static> = Env::from_raw_vm(vm)?;
					let mut slot: MutexGuard<'_, Option<Agent>> =
						AGENT.lock().map_err(|_| Error::Jvmti(sys::JVMTI_ERROR_INTERNAL))?;
					if slot.is_some() {
						return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
					}
					*slot = Some(Agent {
						env: Some(env),
						shutdown: Shutdown::new(&GATE),
					});
					drop(slot);
					env.set_event_callbacks(&sys::jvmtiEventCallbacks {
						VMInit: Some(vm_init),
						ThreadStart: Some(thread_start),
						..Default::default()
					})?;
					env.set_event_enabled(sys::JVMTI_EVENT_VM_INIT, true)
				})()
			};
			if result.is_ok() {
				sys::JNI_OK
			} else {
				failed();
				sys::JNI_ERR
			}
		},
		failed,
	)
}
