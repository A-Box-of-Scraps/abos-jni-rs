//! A minimal JVMTI event agent for VM initialization and platform-thread events.
#![allow(unsafe_code, reason = "Agent exports and callbacks are native JVMTI entry points.")]

use std::{
	ffi::{c_char, c_void},
	io::Write,
	panic::{AssertUnwindSafe, catch_unwind},
	sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use abos_jvmti::{Allocation, Env, Error, JvmtiVersion, Result, sys};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicUsize = AtomicUsize::new(0);
static ENDED: AtomicUsize = AtomicUsize::new(0);

fn report(message: std::fmt::Arguments<'_>) {
	// A closed output stream must not panic across the native callback boundary.
	let _result: std::io::Result<()> = writeln!(std::io::stderr().lock(), "{message}");
}

fn callback(body: impl FnOnce() -> Result<()>) {
	match catch_unwind(AssertUnwindSafe(body)) {
		Ok(Ok(())) => {}
		Ok(Err(error)) => {
			FAILED.store(true, Ordering::SeqCst);
			report(format_args!("JVMTI_AGENT_ERROR: {error}"));
		}
		Err(_) => {
			FAILED.store(true, Ordering::SeqCst);
			report(format_args!("JVMTI_AGENT_ERROR: callback panicked"));
		}
	}
}

unsafe extern "system" fn vm_init(raw: *mut sys::jvmtiEnv, _jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	callback(|| {
		// SAFETY: This callback receives the agent's JVMTI environment on an
		// attached thread in the live phase. All handles and allocations remain local.
		unsafe {
			let env: Env<'_> = Env::from_raw(raw)?;
			if env.phase()? != sys::JVMTI_PHASE_LIVE || !env.capabilities()?.can_tag_objects() {
				return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
			}
			let mut bytes: Allocation<'_, '_> = env.allocate(16)?;
			bytes.copy_from_slice(b"JVMTI allocation");
			drop(bytes);
			report(format_args!("JVMTI_VM_INIT version={:#x}", env.version().as_raw()));
		}
		INITIALIZED.store(true, Ordering::SeqCst);
		Ok(())
	});
}

unsafe extern "system" fn thread_start(_env: *mut sys::jvmtiEnv, _jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	STARTED.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn thread_end(_env: *mut sys::jvmtiEnv, _jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	ENDED.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn vm_death(_env: *mut sys::jvmtiEnv, _jni: *mut sys::JNIEnv) {
	callback(|| {
		let started: usize = STARTED.load(Ordering::SeqCst);
		let ended: usize = ENDED.load(Ordering::SeqCst);
		if FAILED.load(Ordering::SeqCst) || !INITIALIZED.load(Ordering::SeqCst) || started == 0 || ended == 0 {
			return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
		}
		report(format_args!("JVMTI_AGENT_OK thread_start={started} thread_end={ended}"));
		Ok(())
	});
}

unsafe fn configure(env: Env<'_>) -> Result<()> {
	// SAFETY: Agent_OnLoad permits capability acquisition and event setup. Callback
	// code lives in this agent library and remains loaded throughout VM execution.
	unsafe {
		let potential: sys::jvmtiCapabilities = env.potential_capabilities()?;
		if !potential.can_tag_objects() {
			return Err(Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE));
		}
		let mut requested: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		requested.set_can_tag_objects(true);
		env.add_capabilities(&requested)?;
		let callbacks = sys::jvmtiEventCallbacks {
			VMInit: Some(vm_init),
			VMDeath: Some(vm_death),
			ThreadStart: Some(thread_start),
			ThreadEnd: Some(thread_end),
			..Default::default()
		};
		env.set_event_callbacks(&callbacks)?;
		for event in [
			sys::JVMTI_EVENT_VM_INIT,
			sys::JVMTI_EVENT_VM_DEATH,
			sys::JVMTI_EVENT_THREAD_START,
			sys::JVMTI_EVENT_THREAD_END,
		] {
			env.set_event_enabled(event, true)?;
		}
	}
	Ok(())
}

unsafe fn verify_connections(vm: *mut sys::JavaVM, primary: Env<'_>) -> Result<()> {
	// SAFETY: Called during OnLoad with a live VM. Each successful constructor
	// creates its own connection; the secondary connection has no callbacks.
	unsafe {
		let secondary: Env<'_> = Env::from_raw_vm_with_version(vm, JvmtiVersion::V1_2)?;
		let independent: bool = secondary.as_raw() != primary.as_raw() && !secondary.capabilities()?.can_tag_objects();
		secondary.dispose()?;
		if !independent {
			return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
		}
		let future: JvmtiVersion = JvmtiVersion::from_raw(0x3fff_0000)?;
		match Env::from_raw_vm_with_version(vm, future) {
			Err(Error::Jni(sys::JNI_EVERSION)) => {}
			Ok(unexpected) => {
				unexpected.dispose()?;
				return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
			}
			Err(error) => return Err(error),
		}
	}
	report(format_args!("JVMTI_COMPAT_OK version={}", primary.version()));
	Ok(())
}

/// Initializes the example agent when the JVM loads the native library.
///
/// # Safety
/// The JVM must supply a valid VM pointer in the `OnLoad` phase, and keep the agent
/// library loaded until callbacks can no longer execute.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Agent_OnLoad(
	vm: *mut sys::JavaVM,
	_options: *mut c_char,
	_reserved: *mut c_void,
) -> sys::jint {
	let result: std::thread::Result<Result<()>> = catch_unwind(|| {
		// SAFETY: This export is called by the JVM in the `OnLoad` phase. No environment
		// users exist on setup failure; successful setup leaves the environment alive.
		unsafe {
			let env: Env<'_> = Env::from_raw_vm(vm)?;
			if let Err(error) = configure(env).and_then(|()| verify_connections(vm, env)) {
				let _cleanup: Result<()> = env.dispose();
				return Err(error);
			}
		}
		Ok(())
	});
	match result {
		Ok(Ok(())) => sys::JNI_OK,
		Ok(Err(error)) => {
			report(format_args!("JVMTI_AGENT_ERROR: {error}"));
			sys::JNI_ERR
		}
		Err(_) => sys::JNI_ERR,
	}
}
