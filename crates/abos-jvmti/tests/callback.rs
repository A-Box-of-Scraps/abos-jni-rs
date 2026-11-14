//! Panic boundaries and callback-specific fallback behavior, without a JVM.

use std::{
	cell::Cell,
	panic::panic_any,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	},
};

use abos_jvmti::{catch_callback, catch_callback_with, sys};

struct PanicOnDrop(Arc<AtomicUsize>);

impl Drop for PanicOnDrop {
	fn drop(&mut self) {
		self.0.fetch_add(1, Ordering::SeqCst);
		panic!("destructor panicked");
	}
}

#[test]
fn success_returns_body_result_without_running_handler() {
	let body_calls: Cell<usize> = Cell::new(0);
	let handler_calls: Cell<usize> = Cell::new(0);
	assert_eq!(catch_callback(-1, || 42), 42);
	assert_eq!(
		catch_callback_with(
			-1,
			|| {
				body_calls.set(body_calls.get() + 1);
				42
			},
			|| handler_calls.set(handler_calls.get() + 1)
		),
		42
	);
	assert_eq!(body_calls.get(), 1);
	assert_eq!(handler_calls.get(), 0);
}

#[test]
fn body_panic_returns_explicit_fallback_and_runs_handler_once() {
	let handler_calls: Cell<usize> = Cell::new(0);
	assert_eq!(catch_callback(17, || panic!("body panicked")), 17);
	assert_eq!(
		catch_callback_with(
			23,
			|| panic!("body panicked"),
			|| {
				handler_calls.set(handler_calls.get() + 1);
			}
		),
		23
	);
	assert_eq!(handler_calls.get(), 1);
}

#[test]
fn ordinary_errors_are_handled_inside_the_boundary() {
	let reported: Cell<bool> = Cell::new(false);
	let panicked: Cell<bool> = Cell::new(false);
	let result: sys::jint = catch_callback_with(
		sys::JNI_ERR,
		|| {
			let work: Result<(), &str> = Err("setup failed");
			if work.is_ok() {
				sys::JNI_OK
			} else {
				reported.set(true);
				sys::JNI_ERR
			}
		},
		|| panicked.set(true),
	);
	assert_eq!(result, sys::JNI_ERR);
	assert!(reported.get());
	assert!(!panicked.get());

	assert_eq!(
		catch_callback_with(
			19,
			|| {
				// Error formatting/reporting belongs inside the body, not after the boundary.
				panic!("error reporter panicked");
			},
			|| panicked.set(true)
		),
		19
	);
	assert!(panicked.get());
}

#[test]
fn handler_panic_returns_fallback() {
	assert_eq!(
		catch_callback_with(31, || panic!("body panicked"), || panic!("handler panicked")),
		31
	);
}

#[test]
fn hostile_body_payload_is_never_dropped() {
	let drops: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
	assert_eq!(catch_callback(11, || panic_any(PanicOnDrop(Arc::clone(&drops)))), 11);
	let handled: Cell<bool> = Cell::new(false);
	assert_eq!(
		catch_callback_with(13, || panic_any(PanicOnDrop(Arc::clone(&drops))), || handled.set(true)),
		13
	);
	assert!(handled.get());
	assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn hostile_handler_payload_is_never_dropped() {
	let drops: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
	assert_eq!(
		catch_callback_with(
			29,
			|| panic!("body panicked"),
			|| panic_any(PanicOnDrop(Arc::clone(&drops)))
		),
		29
	);
	assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn unused_handler_capture_drop_is_inside_boundary() {
	let drops: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
	let capture: PanicOnDrop = PanicOnDrop(Arc::clone(&drops));
	let handled: Cell<bool> = Cell::new(false);
	assert_eq!(
		catch_callback_with(
			-1,
			|| 42,
			|| {
				handled.set(true);
				drop(capture);
			}
		),
		-1
	);
	assert!(!handled.get());
	assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn body_capture_drop_is_inside_boundary() {
	let drops: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
	let capture: PanicOnDrop = PanicOnDrop(Arc::clone(&drops));
	assert_eq!(
		catch_callback(-1, || {
			drop(capture);
			42
		}),
		-1
	);
	assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn boundary_does_not_roll_back_partial_work() {
	let progress: Cell<usize> = Cell::new(0);
	assert_eq!(
		catch_callback(-1, || {
			progress.set(1);
			panic!("failed after mutation");
		}),
		-1
	);
	assert_eq!(progress.get(), 1);
}

#[test]
fn native_callbacks_return_event_specific_fallbacks_without_a_jvm() {
	static FAILED: AtomicBool = AtomicBool::new(false);

	extern "system" fn notification(_env: *mut sys::jvmtiEnv) {
		catch_callback_with(
			(),
			|| panic!("notification panicked"),
			|| {
				FAILED.store(true, Ordering::SeqCst);
			},
		);
	}

	extern "system" fn heap_visitor(
		_class_tag: sys::jlong,
		_size: sys::jlong,
		_tag: *mut sys::jlong,
		_length: sys::jint,
		_data: *mut std::ffi::c_void,
	) -> sys::jint {
		catch_callback(sys::JVMTI_VISIT_ABORT, || panic!("heap visitor panicked"))
	}

	extern "system" fn legacy_visitor(
		_class_tag: sys::jlong,
		_size: sys::jlong,
		_tag: *mut sys::jlong,
		_data: *mut std::ffi::c_void,
	) -> sys::jvmtiIterationControl {
		catch_callback(sys::JVMTI_ITERATION_ABORT, || panic!("legacy visitor panicked"))
	}

	extern "system" fn on_load(
		_vm: *mut sys::JavaVM,
		_options: *mut std::ffi::c_char,
		_reserved: *mut std::ffi::c_void,
	) -> sys::jint {
		catch_callback(sys::JNI_ERR, || panic!("agent initialization panicked"))
	}

	// No VM or thread attachment exists. These native entry points must contain the
	// panic themselves; unwinding out of extern "system" would terminate the test.
	notification(std::ptr::null_mut());
	assert!(FAILED.load(Ordering::SeqCst));
	assert_eq!(
		heap_visitor(0, 0, std::ptr::null_mut(), 0, std::ptr::null_mut()),
		sys::JVMTI_VISIT_ABORT
	);
	assert_eq!(
		legacy_visitor(0, 0, std::ptr::null_mut(), std::ptr::null_mut()),
		sys::JVMTI_ITERATION_ABORT
	);
	assert_eq!(
		on_load(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()),
		sys::JNI_ERR
	);
}

#[test]
fn boundaries_are_reentrant_and_have_no_shared_failure_state() {
	std::thread::scope(|scope| {
		for _ in 0..4 {
			scope.spawn(|| {
				let handled: Cell<bool> = Cell::new(false);
				assert_eq!(
					catch_callback_with(
						-1,
						|| {
							assert_eq!(catch_callback(-2, || panic!("inner callback panicked")), -2);
							42
						},
						|| handled.set(true)
					),
					42
				);
				assert!(!handled.get());
			});
		}
	});
}
