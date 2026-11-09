use crate::jvmtiCapabilities;

macro_rules! capability {
	($get:ident, $set:ident, $bit:literal) => {
		#[must_use]
		pub const fn $get(&self) -> bool {
			self.words[$bit / 32] & Self::mask($bit) != 0
		}

		pub const fn $set(&mut self, enabled: bool) {
			let mask: u32 = Self::mask($bit);
			if enabled {
				self.words[$bit / 32] |= mask;
			} else {
				self.words[$bit / 32] &= !mask;
			}
		}
	};
}

impl jvmtiCapabilities {
	const fn mask(bit: usize) -> u32 {
		// C allocates bitfields from the opposite end of a storage unit on
		// big-endian targets. Named capabilities occupy consecutive one-bit fields.
		let shift: usize = if cfg!(target_endian = "little") {
			bit % 32
		} else {
			31 - bit % 32
		};
		1 << shift
	}
}

#[path = "capability_bits.rs"]
mod bits;
