use super::decode_modified_utf8;

#[test]
fn modified_utf8() {
	assert_eq!(decode_modified_utf8(b"ASCII"), Some("ASCII".into()));
	assert_eq!(decode_modified_utf8(&[0xc0, 0x80]), Some("\0".into()));
	assert_eq!(decode_modified_utf8("é水".as_bytes()), Some("é水".into()));
	assert_eq!(
		decode_modified_utf8(&[0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80]),
		Some("😀".into())
	);
	for invalid in [
		&[0][..],
		&[0x80],
		&[0xc1, 0x81],
		&[0xe0, 0x80, 0x80],
		&[0xc2],
		&[0xe1, 0x80],
		&[0xc2, 0x41],
		&[0xed, 0xa0, 0x80],
		"😀".as_bytes(),
	] {
		assert_eq!(decode_modified_utf8(invalid), None, "{invalid:?}");
	}
}
