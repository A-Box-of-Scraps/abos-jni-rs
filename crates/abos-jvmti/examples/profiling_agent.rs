//! `HotSpot`-only compiled-method profiling agent.
//!
//! Build with `cargo build -p abos-jvmti --example profiling_agent`, then launch
//! `HotSpot` with `-agentpath:/absolute/path/to/libprofiling_agent.so=hotspot` (use
//! the platform's library suffix). The explicit option asserts the metadata ABI;
//! do not use it with a VM that does not implement `jvmticmlr.h` version 1.0.
//!
//! Emits PC/method/BCI mappings without calling JNI or Java from the callback.
//! A production profiler should resolve symbols while classes are live and track
//! `CompiledMethodUnload` to retire code ranges. Copied IDs do not pin code/classes.
#![allow(unsafe_code, reason = "Native JVMTI agent entry points and callback metadata.")]

use std::{
	ffi::{CStr, c_char, c_void},
	io::Write,
	panic::{AssertUnwindSafe, catch_unwind},
};

use abos_jvmti::{CompiledMethodError, CompiledMethodRecords, Env, OwnedCompiledMethodRecord, Result, sys};

fn report(message: std::fmt::Arguments<'_>) {
	let _result: std::io::Result<()> = writeln!(std::io::stderr().lock(), "{message}");
}

unsafe extern "system" fn compiled_method_load(
	_env: *mut sys::jvmtiEnv,
	method: sys::jmethodID,
	code_size: sys::jint,
	code_addr: *const c_void,
	_map_length: sys::jint,
	_map: *const sys::jvmtiAddrLocationMap,
	compile_info: *const c_void,
) {
	let result: std::thread::Result<()> = catch_unwind(AssertUnwindSafe(|| {
		// SAFETY: The agent is explicitly opted into HotSpot's metadata ABI, and
		// all decoding/copying occurs before this CompiledMethodLoad returns.
		let records: std::result::Result<Vec<OwnedCompiledMethodRecord>, CompiledMethodError> = unsafe {
			CompiledMethodRecords::with(compile_info, |records| {
				records
					.map(|record| record.map(|value| value.to_owned()))
					.collect::<std::result::Result<Vec<_>, _>>()
			})
		};
		match records {
			Ok(records) => {
				report(format_args!(
					"JVMTI_COMPILED method={method:p} start={code_addr:p} size={code_size}"
				));
				// These deep copies can outlive the decoding scope. No callback array
				// pointers escape, and no copied method IDs are dereferenced here.
				for record in &records {
					report_record(record);
				}
			}
			Err(error) => report(format_args!("JVMTI_PROFILE_ERROR: {error}")),
		}
	}));
	if result.is_err() {
		report(format_args!("JVMTI_PROFILE_ERROR: callback panicked"));
	}
}

fn report_record(record: &OwnedCompiledMethodRecord) {
	match record {
		OwnedCompiledMethodRecord::Inline(pcs) => {
			for pc in pcs {
				for frame in &pc.frames {
					report(format_args!(
						"JVMTI_INLINE pc={:#x} method={:p} bci={}",
						pc.pc, frame.method, frame.bci
					));
				}
			}
		}
		other => report(format_args!("JVMTI_METADATA {other:?}")),
	}
}

unsafe fn configure(env: Env<'_>) -> Result<()> {
	// SAFETY: Called during OnLoad; callback code remains loaded for the VM lifetime.
	unsafe {
		let mut capabilities: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
		capabilities.set_can_generate_compiled_method_load_events(true);
		env.add_capabilities(&capabilities)?;
		env.set_event_callbacks(&sys::jvmtiEventCallbacks {
			CompiledMethodLoad: Some(compiled_method_load),
			..Default::default()
		})?;
		env.set_event_enabled(sys::JVMTI_EVENT_COMPILED_METHOD_LOAD, true)
	}
}

/// Initialize a profiling agent explicitly configured for `HotSpot` metadata.
///
/// # Safety
/// The VM must provide valid `OnLoad` arguments and keep this library loaded while
/// callbacks can execute. Non-null options must be a readable C string.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Agent_OnLoad(
	vm: *mut sys::JavaVM,
	options: *mut c_char,
	_reserved: *mut c_void,
) -> sys::jint {
	let result: std::thread::Result<Result<bool>> = catch_unwind(|| {
		// SAFETY: Native OnLoad arguments are valid for this call.
		unsafe {
			if options.is_null() || CStr::from_ptr(options) != c"hotspot" {
				report(format_args!(
					"profiling_agent requires =hotspot on a jvmticmlr.h-compatible VM"
				));
				return Ok(false);
			}
			let env: Env<'_> = Env::from_raw_vm(vm)?;
			if let Err(error) = configure(env) {
				let _cleanup: Result<()> = env.dispose();
				return Err(error);
			}
			Ok(true)
		}
	});
	match result {
		Ok(Ok(true)) => sys::JNI_OK,
		Ok(Err(error)) => {
			report(format_args!("JVMTI_PROFILE_ERROR: {error}"));
			sys::JNI_ERR
		}
		_ => sys::JNI_ERR,
	}
}
