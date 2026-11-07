use super::reader::Reader;
use crate::Result;

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
			if matches!(tag, 5 | 6) && entries.len() + 1 >= count {
				return Err(reader.error("missing second constant-pool slot for long or double"));
			}
			let entry: Constant<'a> = match tag {
				1 => {
					let length: usize = usize::from(reader.u16()?);
					Constant::Utf8(reader.take(length)?)
				}
				7 => Constant::Class(reader.u16()?),
				3 | 4 | 9..=12 | 17 | 18 => {
					reader.take(4)?;
					Constant::Unused
				}
				5 | 6 => {
					// Long and double reserve the next index, even at the end of the pool.
					reader.take(8)?;
					entries.push(Constant::Unused);
					Constant::Unused
				}
				8 | 16 | 19 | 20 => {
					reader.take(2)?;
					Constant::Unused
				}
				15 => {
					reader.take(3)?;
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
			1..=0x7f => u16::from(first),
			0xc0..=0xdf => {
				let second: u16 = continuation(&mut bytes)?;
				let unit: u16 = (u16::from(first & 0x1f) << 6) | second;
				if unit != 0 && unit < 0x80 {
					return None;
				}
				unit
			}
			0xe0..=0xef => {
				let second: u16 = continuation(&mut bytes)?;
				let third: u16 = continuation(&mut bytes)?;
				let unit: u16 = (u16::from(first & 0x0f) << 12) | (second << 6) | third;
				if unit < 0x800 {
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
	(byte & 0xc0 == 0x80).then_some(u16::from(byte & 0x3f))
}

#[cfg(test)]
#[path = "../../tests/unit/pool.rs"]
mod tests;
