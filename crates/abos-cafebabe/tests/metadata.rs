//! Synthetic class-file layout and malformed-input regression tests.
use abos_cafebabe::{AccessFlags, ClassFile, parse_class};

fn words(bytes: &mut Vec<u8>, values: &[u16]) {
	for value in values {
		bytes.extend(value.to_be_bytes());
	}
}

fn utf8(bytes: &mut Vec<u8>, value: &[u8]) {
	bytes.push(1);
	words(bytes, &[u16::try_from(value.len()).expect("test constant length")]);
	bytes.extend(value);
}

fn fixture(major: u16, minor: u16) -> Vec<u8> {
	fixture_with_constants(major, minor, &[], 0)
}

fn fixture_with_constants(major: u16, minor: u16, constants: &[u8], slots: u16) -> Vec<u8> {
	let mut bytes: Vec<u8> = 0xCAFE_BABE_u32.to_be_bytes().to_vec();
	words(&mut bytes, &[minor, major, 12 + slots]);
	utf8(&mut bytes, b"sample/Test"); // 1
	bytes.push(7);
	words(&mut bytes, &[1]); // 2: class
	utf8(&mut bytes, b"java/lang/Object"); // 3
	bytes.push(7);
	words(&mut bytes, &[3]); // 4: superclass
	utf8(&mut bytes, b"java/lang/Runnable"); // 5
	bytes.push(7);
	words(&mut bytes, &[5]); // 6: interface
	utf8(&mut bytes, b"value"); // 7
	utf8(&mut bytes, b"I"); // 8
	utf8(&mut bytes, b"run"); // 9
	utf8(&mut bytes, b"()V"); // 10
	utf8(&mut bytes, b"Opaque"); // 11
	bytes.extend(constants);
	words(&mut bytes, &[0x21, 2, 4, 1, 6, 1]);
	words(&mut bytes, &[0x19, 7, 8, 1]);
	attribute(&mut bytes);
	words(&mut bytes, &[1, 0x101, 9, 10, 1]);
	attribute(&mut bytes);
	words(&mut bytes, &[1]);
	attribute(&mut bytes);
	bytes
}

fn attribute(bytes: &mut Vec<u8>) {
	words(bytes, &[11]);
	bytes.extend(3_u32.to_be_bytes());
	bytes.extend([0xff, 0, 0x42]);
}

#[test]
fn versions_and_metadata() {
	for major in 45..=71 {
		let class: ClassFile = parse_class(&fixture(major, 0)).expect("supported version");
		assert_eq!(class.major_version, major);
		check_metadata(&class);
	}
	assert_eq!(
		parse_class(&fixture(71, 65535)).expect("Java 27 preview").minor_version,
		65535
	);
	assert!(parse_class(&fixture(55, 42)).is_ok());
	assert!(parse_class(&fixture(56, 0)).is_ok());
	assert!(parse_class(&fixture(56, 65535)).is_ok());
	for (major, minor) in [(44, 0), (72, 0), (56, 1), (56, 65534), (71, 1)] {
		assert!(parse_class(&fixture(major, minor)).is_err());
	}
}

fn check_metadata(class: &ClassFile) {
	assert_eq!(class.this_class, "sample/Test");
	assert_eq!(class.super_class.as_deref(), Some("java/lang/Object"));
	assert_eq!(class.interfaces, ["java/lang/Runnable"]);
	assert_eq!(class.fields[0].name, "value");
	assert_eq!(class.fields[0].descriptor, "I");
	assert!(class.fields[0].access_flags.contains(AccessFlags::STATIC));
	assert!(class.fields[0].access_flags.contains(AccessFlags::FINAL));
	assert_eq!(class.methods[0].name, "run");
	assert_eq!(class.methods[0].descriptor, "()V");
	assert!(class.methods[0].access_flags.contains(AccessFlags::NATIVE));
}

#[test]
fn truncation_and_mutation_never_panic() {
	let bytes: Vec<u8> = fixture(71, 0);
	for length in 0..bytes.len() {
		assert!(parse_class(&bytes[..length]).is_err(), "truncated at {length}");
	}
	// Deterministic mutation sweep exercises lengths, tags, indices and UTF-8.
	for index in 0..bytes.len() {
		for value in 0..=255 {
			let mut mutated: Vec<u8> = bytes.clone();
			mutated[index] = value;
			let _result: Result<ClassFile, abos_cafebabe::ParseError> = parse_class(&mutated);
		}
	}
}

#[test]
fn rejects_invalid_structure() {
	let bytes: Vec<u8> = fixture(71, 0);
	let mut invalid: Vec<u8> = bytes.clone();
	invalid.push(0);
	assert!(parse_class(&invalid).is_err());
	for (offset, replacement) in [(0, 0), (9, 0), (10, 2)] {
		invalid.clone_from(&bytes);
		invalid[offset] = replacement;
		assert!(parse_class(&invalid).is_err());
	}
	invalid.clone_from(&bytes);
	let length_offset: usize = invalid.len() - 7;
	invalid[length_offset..length_offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
	assert!(parse_class(&invalid).is_err());
}

#[test]
fn all_constant_tags_and_two_slot_entries() {
	for (tag, size) in [
		(3, 4),
		(4, 4),
		(5, 8),
		(6, 8),
		(8, 2),
		(9, 4),
		(10, 4),
		(11, 4),
		(12, 4),
		(15, 3),
		(16, 2),
		(17, 4),
		(18, 4),
		(19, 2),
		(20, 2),
	] {
		// Append unused constants so existing metadata indices remain unchanged.
		let mut constant: Vec<u8> = vec![tag];
		constant.resize(1 + size, 0);
		let slots: u16 = if matches!(tag, 5 | 6) { 2 } else { 1 };
		let mut bytes: Vec<u8> = fixture_with_constants(71, 0, &constant, slots);
		assert!(parse_class(&bytes).is_ok(), "tag {tag}");
		if slots == 2 {
			bytes[8..10].copy_from_slice(&13_u16.to_be_bytes());
			assert!(parse_class(&bytes).is_err());
		}
	}
}

#[test]
fn references_after_wide_constants_and_invalid_indices() {
	let mut bytes: Vec<u8> = 0xCAFE_BABE_u32.to_be_bytes().to_vec();
	words(&mut bytes, &[0, 71, 7]);
	bytes.push(5); // indices 1 and 2
	bytes.extend(0_u64.to_be_bytes());
	bytes.push(6); // indices 3 and 4
	bytes.extend(0_u64.to_be_bytes());
	utf8(&mut bytes, b"java/lang/Object"); // index 5
	bytes.push(7);
	words(&mut bytes, &[5]); // index 6
	let body: usize = bytes.len();
	words(&mut bytes, &[0x21, 6, 0, 0, 0, 0, 0]);
	let class: ClassFile = parse_class(&bytes).expect("references following wide constants");
	assert_eq!(class.this_class, "java/lang/Object");
	assert!(class.super_class.is_none());
	for invalid in [0_u16, 1, 2, 3, 4, 5, 7, u16::MAX] {
		bytes[body + 2..body + 4].copy_from_slice(&invalid.to_be_bytes());
		assert!(parse_class(&bytes).is_err(), "class index {invalid}");
	}
	bytes[body + 2..body + 4].copy_from_slice(&6_u16.to_be_bytes());
	for invalid in [0_u16, 1, 6, 7, u16::MAX] {
		bytes[body - 2..body].copy_from_slice(&invalid.to_be_bytes());
		assert!(parse_class(&bytes).is_err(), "name index {invalid}");
	}
}
