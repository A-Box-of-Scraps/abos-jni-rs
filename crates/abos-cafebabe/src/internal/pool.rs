use super::reader::Reader;
use crate::Result;

const CONSTANT_UTF8: u8 = 1;
const CONSTANT_INTEGER: u8 = 3;
const CONSTANT_FLOAT: u8 = 4;
const CONSTANT_LONG: u8 = 5;
const CONSTANT_DOUBLE: u8 = 6;
const CONSTANT_CLASS: u8 = 7;
const CONSTANT_STRING: u8 = 8;
const CONSTANT_FIELD_REF: u8 = 9;
const CONSTANT_METHOD_REF: u8 = 10;
const CONSTANT_INTERFACE_METHOD_REF: u8 = 11;
const CONSTANT_NAME_AND_TYPE: u8 = 12;
const CONSTANT_METHOD_HANDLE: u8 = 15;
const CONSTANT_METHOD_TYPE: u8 = 16;
const CONSTANT_DYNAMIC: u8 = 17;
const CONSTANT_INVOKE_DYNAMIC: u8 = 18;
const CONSTANT_MODULE: u8 = 19;
const CONSTANT_PACKAGE: u8 = 20;

const UTF8_NONZERO_ASCII_MIN: u8 = 0x01;
const UTF8_ASCII_MAX: u8 = 0x7f;
const UTF8_TWO_BYTE_LEAD_MIN: u8 = 0xc0;
const UTF8_TWO_BYTE_LEAD_MAX: u8 = 0xdf;
const UTF8_THREE_BYTE_LEAD_MIN: u8 = 0xe0;
const UTF8_THREE_BYTE_LEAD_MAX: u8 = 0xef;
const UTF8_TWO_BYTE_LEAD_PAYLOAD_MASK: u8 = 0x1f;
const UTF8_THREE_BYTE_LEAD_PAYLOAD_MASK: u8 = 0x0f;
const UTF8_CONTINUATION_TAG_MASK: u8 = 0xc0;
const UTF8_CONTINUATION_TAG: u8 = 0x80;
const UTF8_CONTINUATION_PAYLOAD_MASK: u8 = 0x3f;
const UTF8_CONTINUATION_PAYLOAD_BITS: u32 = 6;
const UTF8_TWO_BYTE_MIN: u16 = 0x80;
const UTF8_THREE_BYTE_MIN: u16 = 0x800;

#[derive(Clone, Copy)]
enum Constant<'a> {
	Unused,
	Utf8(&'a [u8]),
	Class(u16),
}

pub(super) struct Pool<'a>(Vec<Constant<'a>>);

impl<'a> Pool<'a> {
	pub(super) fn read(reader: &mut Reader<'a>) -> Result<Self> {
		let count: usize = usize::from(reader.u16()?);
		if count == 0 {
			return Err(reader.error("constant pool count must be nonzero"));
		}
		let mut entries: Vec<Constant<'a>> = vec![Constant::Unused];
		while entries.len() < count {
			let tag: u8 = reader.u8()?;
			if matches!(tag, CONSTANT_LONG | CONSTANT_DOUBLE) && entries.len() + 1 >= count {
				return Err(reader.error("missing second constant-pool slot for long or double"));
			}
			let entry: Constant<'a> = match tag {
				CONSTANT_UTF8 => {
					let length: usize = usize::from(reader.u16()?);
					Constant::Utf8(reader.take(length)?)
				}
				CONSTANT_CLASS => Constant::Class(reader.u16()?),
				CONSTANT_INTEGER
				| CONSTANT_FLOAT
				| CONSTANT_FIELD_REF
				| CONSTANT_METHOD_REF
				| CONSTANT_INTERFACE_METHOD_REF
				| CONSTANT_NAME_AND_TYPE
				| CONSTANT_DYNAMIC
				| CONSTANT_INVOKE_DYNAMIC => {
					reader.take(size_of::<u32>())?;
					Constant::Unused
				}
				CONSTANT_LONG | CONSTANT_DOUBLE => {
					// Long and double reserve the next index, even at the end of the pool.
					reader.take(size_of::<u64>())?;
					entries.push(Constant::Unused);
					Constant::Unused
				}
				CONSTANT_STRING | CONSTANT_METHOD_TYPE | CONSTANT_MODULE | CONSTANT_PACKAGE => {
					reader.take(size_of::<u16>())?;
					Constant::Unused
				}
				CONSTANT_METHOD_HANDLE => {
					reader.take(size_of::<u8>() + size_of::<u16>())?;
					Constant::Unused
				}
				_ => return Err(reader.error("unknown constant-pool tag")),
			};
			entries.push(entry);
		}
		Ok(Self(entries))
	}

	pub(super) fn utf8(&self, index: u16, reader: &Reader<'_>) -> Result<&'a [u8]> {
		match self.0.get(usize::from(index)) {
			Some(Constant::Utf8(bytes)) => Ok(bytes),
			_ => Err(reader.error("expected UTF-8 constant-pool reference")),
		}
	}

	pub(super) fn text(&self, index: u16, reader: &Reader<'_>) -> Result<String> {
		decode_modified_utf8(self.utf8(index, reader)?)
			.ok_or_else(|| reader.error("invalid modified UTF-8 or unpaired surrogate in metadata"))
	}

	pub(super) fn class(&self, index: u16, reader: &Reader<'_>) -> Result<String> {
		match self.0.get(usize::from(index)) {
			Some(Constant::Class(name)) => self.text(*name, reader),
			_ => Err(reader.error("expected class constant-pool reference")),
		}
	}

	pub(super) fn class_name(&self, reader: &mut Reader<'_>) -> Result<String> {
		self.class(reader.u16()?, reader)
	}
}

fn decode_modified_utf8(bytes: &[u8]) -> Option<String> {
	let mut units: Vec<u16> = Vec::new();
	let mut bytes: std::slice::Iter<'_, u8> = bytes.iter();
	while let Some(&first) = bytes.next() {
		let unit: u16 = match first {
			UTF8_NONZERO_ASCII_MIN..=UTF8_ASCII_MAX => u16::from(first),
			UTF8_TWO_BYTE_LEAD_MIN..=UTF8_TWO_BYTE_LEAD_MAX => {
				let second: u16 = continuation(&mut bytes)?;
				let unit: u16 =
					(u16::from(first & UTF8_TWO_BYTE_LEAD_PAYLOAD_MASK) << UTF8_CONTINUATION_PAYLOAD_BITS) | second;
				// Modified UTF-8 permits an overlong encoding only for NUL.
				if unit != 0 && unit < UTF8_TWO_BYTE_MIN {
					return None;
				}
				unit
			}
			UTF8_THREE_BYTE_LEAD_MIN..=UTF8_THREE_BYTE_LEAD_MAX => {
				let second: u16 = continuation(&mut bytes)?;
				let third: u16 = continuation(&mut bytes)?;
				let unit: u16 = (u16::from(first & UTF8_THREE_BYTE_LEAD_PAYLOAD_MASK)
					<< (2 * UTF8_CONTINUATION_PAYLOAD_BITS))
					| (second << UTF8_CONTINUATION_PAYLOAD_BITS)
					| third;
				if unit < UTF8_THREE_BYTE_MIN {
					return None;
				}
				unit
			}
			_ => return None,
		};
		units.push(unit);
	}
	String::from_utf16(&units).ok()
}

fn continuation(bytes: &mut std::slice::Iter<'_, u8>) -> Option<u16> {
	let byte: u8 = *bytes.next()?;
	(byte & UTF8_CONTINUATION_TAG_MASK == UTF8_CONTINUATION_TAG)
		.then_some(u16::from(byte & UTF8_CONTINUATION_PAYLOAD_MASK))
}

#[cfg(test)]
#[path = "../../tests/unit/pool.rs"]
mod tests;
