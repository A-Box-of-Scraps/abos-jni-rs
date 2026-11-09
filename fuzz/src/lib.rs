//! Shared harnesses used by libFuzzer and deterministic corpus tests.

pub mod jvmti_compiled_method;
pub mod jvmti_returned_data;

/// Missing input bytes are zero, keeping short inputs useful without allocations.
struct Input<'a>(&'a [u8]);

impl Input<'_> {
	fn byte(&mut self) -> u8 {
		let Some((&byte, rest)) = self.0.split_first() else {
			return 0;
		};
		self.0 = rest;
		byte
	}

	fn int(&mut self) -> i32 {
		i32::from_le_bytes(std::array::from_fn(|_| self.byte()))
	}
}
