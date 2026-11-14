//! Deterministic shutdown protocol tests independent of JVM scheduling.

use std::{
	sync::{
		atomic::{AtomicUsize, Ordering},
		mpsc,
	},
	time::{Duration, Instant},
};

use abos_jvmti::{Error, Result, catch_callback, catch_callback_with, sys};

#[path = "../examples/shutdown_agent/protocol.rs"]
mod protocol;
use protocol::{Admission, Gate, Operations, Progress, Shutdown};

const FAILURE: Error = Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE);
const TIMEOUT: Duration = Duration::from_secs(5);

struct RecordingOperations<'a> {
	gate: &'a Gate,
	calls: Vec<&'static str>,
	fail_at: Option<&'static str>,
	panic_at: Option<&'static str>,
}

impl<'a> RecordingOperations<'a> {
	const fn new(gate: &'a Gate) -> Self {
		Self {
			gate,
			calls: Vec::new(),
			fail_at: None,
			panic_at: None,
		}
	}

	fn record(&mut self, operation: &'static str) -> Result<()> {
		self.calls.push(operation);
		assert_ne!(self.panic_at, Some(operation), "injected operation panic");
		if self.fail_at == Some(operation) {
			Err(FAILURE)
		} else {
			Ok(())
		}
	}
}

impl Operations for RecordingOperations<'_> {
	fn disable_delivery(&mut self) -> Result<()> {
		assert!(
			self.gate.enter().is_none(),
			"admission must close before disabling delivery"
		);
		self.record("disable")
	}

	fn release_resources(&mut self) -> Result<()> {
		assert!(
			self.gate.drained(),
			"resources must remain live until admitted callbacks finish"
		);
		self.record("release")
	}

	fn dispose(&mut self) -> Result<()> {
		assert!(self.gate.drained());
		assert_eq!(self.calls.last(), Some(&"release"));
		self.record("dispose")
	}
}

#[test]
fn gate_only_drains_when_closed_and_every_admission_has_left() {
	let gate: Gate = Gate::new();
	assert!(!gate.drained(), "an open idle gate is not safe for cleanup");
	let first: Admission<'_> = gate.enter().expect("open gate");
	let second: Admission<'_> = gate.enter().expect("reentrant callback");
	gate.close();
	gate.close();
	assert!(gate.enter().is_none());
	assert!(!gate.drained());
	drop(first);
	assert!(!gate.drained());
	drop(second);
	assert!(gate.drained());
	assert!(gate.enter().is_none(), "shutdown never reopens admission");
}

#[test]
fn disabling_does_not_release_an_outstanding_callback_and_polling_is_idempotent() {
	let gate: Gate = Gate::new();
	let admission: Admission<'_> = gate.enter().expect("open gate");
	let mut shutdown: Shutdown<'_> = Shutdown::new(&gate);
	let mut operations: RecordingOperations<'_> = RecordingOperations::new(&gate);
	for _ in 0..2 {
		assert_eq!(shutdown.poll(&mut operations), Ok(Progress::Pending));
		assert_eq!(operations.calls, ["disable"]);
	}
	drop(admission);
	for _ in 0..2 {
		assert_eq!(shutdown.poll(&mut operations), Ok(Progress::Complete));
		assert_eq!(operations.calls, ["disable", "release", "dispose"]);
	}
}

#[test]
fn callback_delayed_before_admission_cannot_touch_disposed_resources() {
	let gate: Gate = Gate::new();
	let (ready_tx, ready_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
	let (resume_tx, resume_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
	std::thread::scope(|scope| {
		let callback_gate: &Gate = &gate;
		let callback: std::thread::ScopedJoinHandle<'_, bool> = scope.spawn(move || {
			ready_tx.send(()).expect("coordinator alive");
			resume_rx.recv_timeout(TIMEOUT).expect("callback resumed");
			callback_gate.enter().is_some()
		});
		ready_rx
			.recv_timeout(TIMEOUT)
			.expect("callback arrived before admission");
		let mut shutdown: Shutdown<'_> = Shutdown::new(&gate);
		let mut operations: RecordingOperations<'_> = RecordingOperations::new(&gate);
		assert_eq!(shutdown.poll(&mut operations), Ok(Progress::Complete));
		resume_tx.send(()).expect("callback alive");
		assert!(!callback.join().expect("callback joined"));
		assert_eq!(operations.calls, ["disable", "release", "dispose"]);
	});
}

#[test]
fn callback_requested_shutdown_never_waits_for_itself_and_publishes_work_before_drain() {
	let gate: Gate = Gate::new();
	let work: AtomicUsize = AtomicUsize::new(0);
	let (ready_tx, ready_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
	let (resume_tx, resume_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
	std::thread::scope(|scope| {
		let callback_gate: &Gate = &gate;
		let callback_work: &AtomicUsize = &work;
		let callback: std::thread::ScopedJoinHandle<'_, ()> = scope.spawn(move || {
			let _admission: Admission<'_> = callback_gate.enter().expect("open gate");
			callback_gate.close(); // Request only: no JVMTI calls, waiting, or disposal in the callback.
			ready_tx.send(()).expect("coordinator alive");
			resume_rx.recv_timeout(TIMEOUT).expect("callback resumed");
			callback_work.store(42, Ordering::Relaxed);
		});
		ready_rx
			.recv_timeout(TIMEOUT)
			.expect("callback requested closure without blocking");
		let mut shutdown: Shutdown<'_> = Shutdown::new(&gate);
		let mut operations: RecordingOperations<'_> = RecordingOperations::new(&gate);
		assert_eq!(shutdown.poll(&mut operations), Ok(Progress::Pending));
		assert_eq!(operations.calls, ["disable"]);
		resume_tx.send(()).expect("callback alive");
		let deadline: Instant = Instant::now() + TIMEOUT;
		while !gate.drained() {
			assert!(Instant::now() < deadline, "admission failed to drain");
			std::thread::yield_now();
		}
		// Check before joining: the gate's acquire/release must publish callback writes.
		assert_eq!(work.load(Ordering::Relaxed), 42);
		assert_eq!(shutdown.poll(&mut operations), Ok(Progress::Complete));
		callback.join().expect("callback joined");
	});
}

#[test]
fn callback_unwinding_releases_admission() {
	let gate: Gate = Gate::new();
	catch_callback_with(
		(),
		|| {
			let _admission: Admission<'_> = gate.enter().expect("open gate");
			panic!("callback failed");
		},
		|| gate.close(),
	);
	assert!(gate.drained());
}

#[test]
fn callback_admission_can_cover_panic_reporting_as_well_as_the_body() {
	let gate: Gate = Gate::new();
	let admission: Admission<'_> = gate.enter().expect("open gate");
	catch_callback_with(
		(),
		|| panic!("callback failed"),
		|| {
			gate.close();
			assert!(!gate.drained(), "cleanup must not race failure reporting");
		},
	);
	assert!(!gate.drained());
	drop(admission);
	assert!(gate.drained());
}

#[test]
fn errors_stop_cleanup_and_are_not_automatically_retried() {
	for (index, operation) in ["disable", "release", "dispose"].into_iter().enumerate() {
		let gate: Gate = Gate::new();
		let mut shutdown: Shutdown<'_> = Shutdown::new(&gate);
		let mut operations: RecordingOperations<'_> = RecordingOperations::new(&gate);
		operations.fail_at = Some(operation);
		assert_eq!(shutdown.poll(&mut operations), Err(FAILURE));
		assert!(gate.enter().is_none());
		operations.fail_at = None;
		assert_eq!(shutdown.poll(&mut operations), Err(FAILURE));
		assert_eq!(operations.calls, ["disable", "release", "dispose"][..=index]);
	}
}

#[test]
fn panics_poison_shutdown_before_any_external_operation() {
	let internal_error: Error = Error::Jvmti(sys::JVMTI_ERROR_INTERNAL);
	for (index, operation) in ["disable", "release", "dispose"].into_iter().enumerate() {
		let gate: Gate = Gate::new();
		let mut shutdown: Shutdown<'_> = Shutdown::new(&gate);
		let mut operations: RecordingOperations<'_> = RecordingOperations::new(&gate);
		operations.panic_at = Some(operation);
		assert_eq!(
			catch_callback(Err(internal_error), || shutdown.poll(&mut operations)),
			Err(internal_error)
		);
		assert!(gate.enter().is_none());
		operations.panic_at = None;
		assert_eq!(shutdown.poll(&mut operations), Err(internal_error));
		assert_eq!(operations.calls, ["disable", "release", "dispose"][..=index]);
	}
}
