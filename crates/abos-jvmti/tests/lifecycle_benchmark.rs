//! The lifecycle benchmark must bound state growth independently of Criterion's iteration count.
use std::{cell::RefCell, time::Duration};

#[path = "../benches/support/lifecycle.rs"]
mod lifecycle;

#[test]
fn every_fixed_size_batch_is_surrounded_by_resets() {
	for include_reset in [false, true] {
		let events: RefCell<Vec<&str>> = RefCell::new(Vec::new());
		let _elapsed: Duration = lifecycle::measure_batches(
			3,
			2,
			include_reset,
			|| events.borrow_mut().push("operation"),
			|| events.borrow_mut().push("reset"),
		);
		assert_eq!(
			events.into_inner(),
			[
				"reset",
				"operation",
				"operation",
				"reset",
				"operation",
				"operation",
				"reset",
				"operation",
				"operation",
				"reset"
			]
		);
	}
}

#[test]
fn separate_timing_loop_invocations_start_with_a_reset() {
	let events: RefCell<Vec<&str>> = RefCell::new(Vec::new());
	for _ in 0..2 {
		let _elapsed: Duration = lifecycle::measure_batches(
			1,
			1,
			false,
			|| events.borrow_mut().push("operation"),
			|| events.borrow_mut().push("reset"),
		);
	}
	assert_eq!(
		events.into_inner(),
		["reset", "operation", "reset", "reset", "operation", "reset"]
	);
}

#[test]
fn zero_iterations_only_performs_the_initial_untimed_reset() {
	let mut resets = 0;
	let elapsed: Duration =
		lifecycle::measure_batches(0, 1024, true, || panic!("no operations expected"), || resets += 1);
	assert_eq!(resets, 1);
	assert_eq!(elapsed, Duration::ZERO);
}
