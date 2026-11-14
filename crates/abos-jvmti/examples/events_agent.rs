//! A JVMTI event agent with optional instrumentation and resource-lifecycle exercises.
//!
//! The `features` agent option enables native methods for the bundled Java test
//! fixtures. Without that option the agent only observes VM and thread events
//! and checks environment construction and allocation ownership.
#![allow(unsafe_code, reason = "Agent exports and callbacks are native JVMTI entry points.")]

use std::{
	ffi::{CStr, c_char, c_void},
	io::Write,
	sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use abos_jni::jni_str;
use abos_jvmti::{
	Allocation, Env, Error, JvmtiString, JvmtiStringList, JvmtiVersion, Result, catch_callback, catch_callback_with,
	sys,
};

#[path = "events_agent/features.rs"]
mod features;

static FEATURE_TESTS: AtomicBool = AtomicBool::new(false);
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicUsize = AtomicUsize::new(0);
static ENDED: AtomicUsize = AtomicUsize::new(0);

fn report(message: std::fmt::Arguments<'_>) {
	// A closed output stream must not panic across the native callback boundary.
	let _result: std::io::Result<()> = writeln!(std::io::stderr().lock(), "{message}");
}

fn callback(body: impl FnOnce() -> Result<()>) {
	catch_callback_with(
		(),
		|| {
			if let Err(error) = body() {
				FAILED.store(true, Ordering::SeqCst);
				report(format_args!("JVMTI_AGENT_ERROR: {error}"));
			}
		},
		|| {
			FAILED.store(true, Ordering::SeqCst);
			// This helper is for the example's unrestricted callbacks, not heap/GC callbacks.
			report(format_args!("JVMTI_AGENT_ERROR: callback panicked"));
		},
	);
}

unsafe extern "system" fn vm_init(raw: *mut sys::jvmtiEnv, jni: *mut sys::JNIEnv, _thread: sys::jthread) {
	callback(|| {
		// SAFETY: This callback receives the agent's JVMTI environment on an
		// attached thread in the live phase. All handles and allocations remain local.
		unsafe {
			let env: Env<'_> = Env::from_raw(raw)?;
			if FEATURE_TESTS.load(Ordering::SeqCst) {
				features::register(env, jni)?;
			}
			if env.phase()? != sys::JVMTI_PHASE_LIVE || !env.capabilities()?.can_tag_objects() {
				return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
			}
			let mut bytes: Allocation<'_, '_> = env.allocate(16)?;
			bytes.copy_from_slice(b"JVMTI allocation");
			bytes.free()?;
			let properties: JvmtiStringList<'_, '_> = env.system_properties()?;
			let vm_name: JvmtiString<'_, '_> = env.system_property(jni_str!("java.vm.name"))?;
			if !properties.iter().any(|key| key == jni_str!("java.vm.name")) || vm_name.to_bytes().is_empty() {
				return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
			}
			vm_name.free()?;
			properties.free()?;
			match env.system_property(jni_str!("abos.jvmti.missing.property")) {
				Err(Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE)) => {}
				_ => return Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)),
			}
			report(format_args!("JVMTI_OWNERSHIP_OK"));
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
		if FEATURE_TESTS.load(Ordering::SeqCst) {
			features::request_capabilities(env, &mut requested);
		}
		env.add_capabilities(&requested)?;
		let callbacks = sys::jvmtiEventCallbacks {
			VMInit: Some(vm_init),
			VMDeath: Some(vm_death),
			ThreadStart: Some(thread_start),
			ThreadEnd: Some(thread_end),
			Breakpoint: Some(features::breakpoint),
			SingleStep: Some(features::single_step),
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
/// library loaded until callbacks can no longer execute. Non-null options must
/// point to a readable, null-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Agent_OnLoad(
	vm: *mut sys::JavaVM,
	options: *mut c_char,
	_reserved: *mut c_void,
) -> sys::jint {
	catch_callback(sys::JNI_ERR, || {
		let result: Result<()> = (|| {
			// SAFETY: This export is called by the JVM in the `OnLoad` phase. No environment
			// users exist on setup failure; successful setup leaves the environment alive.
			unsafe {
				let feature_tests: bool = !options.is_null() && CStr::from_ptr(options) == c"features";
				FEATURE_TESTS.store(feature_tests, Ordering::SeqCst);
				let env: Env<'_> = Env::from_raw_vm(vm)?;
				if let Err(error) = configure(env).and_then(|()| verify_connections(vm, env)) {
					let _cleanup: Result<()> = env.dispose();
					return Err(error);
				}
			}
			Ok(())
		})();
		match result {
			Ok(()) => sys::JNI_OK,
			Err(error) => {
				report(format_args!("JVMTI_AGENT_ERROR: {error}"));
				sys::JNI_ERR
			}
		}
	})
}
