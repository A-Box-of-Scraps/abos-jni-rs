#![allow(unsafe_code, reason = "Exercise external JNI detachment without active guards.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use std::{
	cell::Cell,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	},
	thread,
};

use abos_jni::JavaVM;
use log::{LevelFilter, Log, Metadata, Record};
use rusty_fork::rusty_fork_test;

struct TlsLogger;
struct LoggerState;

static LOGGER: TlsLogger = TlsLogger;
static LOG_CALLS: AtomicUsize = AtomicUsize::new(0);
static LOGGER_DESTROYED: AtomicBool = AtomicBool::new(false);
static PANIC_ON_LOG: AtomicBool = AtomicBool::new(false);
static LOGGED_ATTACHMENT_COUNT: AtomicUsize = AtomicUsize::new(usize::MAX);

thread_local! {
	// Delay logger TLS initialization until the permanent attachment is registered.
	static USE_LOGGER_TLS: Cell<bool> = const { Cell::new(false) };
	static LOGGER_STATE: LoggerState = const { LoggerState };
}

impl Drop for LoggerState {
	fn drop(&mut self) {
		#[cfg(use_tls_attach_guard)]
		assert_eq!(util::jvm().threads_attached(), 1, "logger TLS must be destroyed first");
		LOGGER_DESTROYED.store(true, Ordering::SeqCst);
	}
}

impl Log for TlsLogger {
	fn enabled(&self, _: &Metadata<'_>) -> bool {
		true
	}

	fn log(&self, _: &Record<'_>) {
		LOG_CALLS.fetch_add(1, Ordering::SeqCst);
		if PANIC_ON_LOG.swap(false, Ordering::SeqCst) {
			LOGGED_ATTACHMENT_COUNT.store(util::jvm().threads_attached(), Ordering::SeqCst);
			panic!("diagnostic logger panic");
		}
		if USE_LOGGER_TLS.get() {
			// Deliberately use `with`, not `try_with`: accessing destroyed TLS panics,
			// and doing so during automatic JNI teardown can abort the process.
			LOGGER_STATE.with(|_| {});
		}
	}

	fn flush(&self) {}
}

fn install_logger() {
	log::set_logger(&LOGGER).expect("install TLS-dependent logger in subprocess");
	log::set_max_level(LevelFilter::Trace);
}

fn check_teardown_logging(external_detach: bool) {
	let vm: &Arc<JavaVM> = util::jvm();
	vm.detach_current_thread().expect("detach JVM creator");
	assert_eq!(vm.threads_attached(), 0);
	install_logger();

	let log_calls_before_exit: usize = thread::spawn(move || {
		util::attach_current_thread(|_| Ok(())).expect("attach worker");
		assert_eq!(vm.threads_attached(), 1);
		if external_detach {
			// SAFETY: The attachment callback has returned and no guards remain.
			unsafe { util::sys_detach_current_thread() };
			assert!(!util::is_thread_attached());
		}

		USE_LOGGER_TLS.set(true);
		log::debug!("initialize logger TLS after the attachment record");
		assert!(!LOGGER_DESTROYED.load(Ordering::SeqCst));
		LOG_CALLS.load(Ordering::SeqCst)
	})
	.join()
	.expect("worker teardown must not panic or abort");

	assert!(LOGGER_DESTROYED.load(Ordering::SeqCst));
	assert_eq!(vm.threads_attached(), 0, "teardown must finish bookkeeping");
	assert_eq!(LOG_CALLS.load(Ordering::SeqCst), log_calls_before_exit);
}

rusty_fork_test! {
	#[test]
	fn teardown_does_not_log_after_logger_tls_is_destroyed() {
		check_teardown_logging(false);
	}

	#[test]
	fn externally_detached_teardown_does_not_log() {
		check_teardown_logging(true);
	}

	#[test]
	fn explicit_detach_finishes_bookkeeping_before_logging() {
		let vm: &Arc<JavaVM> = util::jvm();
		vm.detach_current_thread().expect("detach JVM creator");
		install_logger();

		thread::spawn(move || {
			util::attach_current_thread(|_| Ok(())).expect("attach worker");
			PANIC_ON_LOG.store(true, Ordering::SeqCst);
			let result: thread::Result<abos_jni::errors::Result<()>> =
				std::panic::catch_unwind(|| vm.detach_current_thread());
			assert!(result.is_err(), "explicit detachment still emits diagnostics");
			assert_eq!(LOGGED_ATTACHMENT_COUNT.load(Ordering::SeqCst), 0);
			assert_eq!(vm.threads_attached(), 0);
			assert!(!util::is_thread_attached());

			// A logger panic must not leave a borrowed or armed cleanup record behind.
			util::attach_current_thread(|_| Ok(())).expect("reattach worker");
			assert_eq!(vm.threads_attached(), 1);
			vm.detach_current_thread().expect("detach worker again");
			assert_eq!(vm.threads_attached(), 0);
		})
		.join()
		.expect("worker recovers from diagnostic panic");
		assert_eq!(vm.threads_attached(), 0);
	}
}
