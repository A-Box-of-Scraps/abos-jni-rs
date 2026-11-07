use super::{pool::Pool, reader::Reader};
use crate::{AccessFlags, ClassFile, Member, Result};

pub fn parse_class(bytes: &[u8]) -> Result<ClassFile> {
	let mut reader: Reader<'_> = Reader::new(bytes);
	if reader.u32()? != 0xCAFE_BABE {
		return Err(reader.error("invalid class-file magic"));
	}
	let minor_version: u16 = reader.u16()?;
	let major_version: u16 = reader.u16()?;
	if !(45..=71).contains(&major_version) {
		return Err(reader.error("unsupported class-file major version (expected 45..=71)"));
	}
	if major_version >= 56 && minor_version != 0 && minor_version != u16::MAX {
		return Err(reader.error("invalid class-file minor version"));
	}
	let pool: Pool<'_> = Pool::read(&mut reader)?;
	let access_flags: AccessFlags = AccessFlags(reader.u16()?);
	let this_class: String = pool.class_name(&mut reader)?;
	let parent_index: u16 = reader.u16()?;
	let super_class: Option<String> = if parent_index == 0 {
		None
	} else {
		Some(pool.class(parent_index, &reader)?)
	};
	let mut interfaces: Vec<String> = Vec::new();
	for _ in 0..reader.u16()? {
		interfaces.push(pool.class_name(&mut reader)?);
	}
	let fields: Vec<Member> = read_members(&mut reader, &pool)?;
	let methods: Vec<Member> = read_members(&mut reader, &pool)?;
	skip_attributes(&mut reader, &pool)?;
	if !reader.is_empty() {
		return Err(reader.error("trailing bytes after class file"));
	}
	Ok(ClassFile {
		major_version,
		minor_version,
		access_flags,
		this_class,
		super_class,
		interfaces,
		fields,
		methods,
	})
}

fn read_members(reader: &mut Reader<'_>, pool: &Pool<'_>) -> Result<Vec<Member>> {
	let mut members: Vec<Member> = Vec::new();
	for _ in 0..reader.u16()? {
		let access_flags: AccessFlags = AccessFlags(reader.u16()?);
		let name: String = pool.text(reader.u16()?, reader)?;
		let descriptor: String = pool.text(reader.u16()?, reader)?;
		skip_attributes(reader, pool)?;
		members.push(Member {
			name,
			descriptor,
			access_flags,
		});
	}
	Ok(members)
}

fn skip_attributes(reader: &mut Reader<'_>, pool: &Pool<'_>) -> Result<()> {
	for _ in 0..reader.u16()? {
		// Attribute names must reference UTF-8 constants, but their payloads are opaque.
		pool.utf8(reader.u16()?, reader)?;
		let length: usize =
			usize::try_from(reader.u32()?).map_err(|_| reader.error("attribute length exceeds address space"))?;
		reader.take(length)?;
	}
	Ok(())
}
