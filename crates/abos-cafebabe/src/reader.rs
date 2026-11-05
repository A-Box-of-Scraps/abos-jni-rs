use crate::{ParseError, Result};

pub struct Reader<'a> {
	bytes: &'a [u8],
	offset: usize,
}

impl<'a> Reader<'a> {
	pub const fn new(bytes: &'a [u8]) -> Self {
		Self { bytes, offset: 0 }
	}

	pub const fn error(&self, message: &'static str) -> ParseError {
		ParseError {
			offset: self.offset,
			message,
		}
	}

	pub const fn is_empty(&self) -> bool {
		self.bytes.is_empty()
	}

	pub const fn take(&mut self, length: usize) -> Result<&'a [u8]> {
		let Some((value, rest)) = self.bytes.split_at_checked(length) else {
			return Err(self.error("truncated class file"));
		};
		self.bytes = rest;
		self.offset += length;
		Ok(value)
	}

	pub fn u8(&mut self) -> Result<u8> {
		Ok(self.take(1)?[0])
	}

	pub fn u16(&mut self) -> Result<u16> {
		let bytes: &[u8] = self.take(2)?;
		Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
	}

	pub fn u32(&mut self) -> Result<u32> {
		let bytes: &[u8] = self.take(4)?;
		Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
	}
}
