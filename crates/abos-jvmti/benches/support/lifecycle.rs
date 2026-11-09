use std::time::{Duration, Instant};

/// Measures fixed-size batches, resetting state before the first and after every batch.
///
/// The initial reset is always outside timing. Subsequent resets are included
/// only when requested. Criterion iteration counts cannot change the batch size.
pub fn measure_batches(
	iterations: u64,
	size: u64,
	include_reset: bool,
	mut operation: impl FnMut(),
	mut reset: impl FnMut(),
) -> Duration {
	reset();
	let mut elapsed: Duration = Duration::ZERO;
	for _ in 0..iterations {
		let start: Instant = Instant::now();
		for _ in 0..size {
			operation();
		}
		let operations: Duration = start.elapsed();
		// Reset after each fixed-size batch, not just between Criterion samples.
		// Otherwise increasing iteration counts would grow the native list again.
		reset();
		elapsed += if include_reset { start.elapsed() } else { operations };
	}
	elapsed
}
