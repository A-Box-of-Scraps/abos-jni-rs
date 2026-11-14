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

#[test]
fn modified_utf8_encoding_boundaries() {
	for text in ["\u{1}", "\u{7f}", "\u{80}", "\u{7ff}", "\u{800}", "\u{ffff}"] {
		assert_eq!(decode_modified_utf8(text.as_bytes()).as_deref(), Some(text));
	}
	for invalid in [
		&[0xc0, 0x81][..],
		&[0xc1, 0xbf],
		&[0xe0, 0x9f, 0xbf],
		&[0xdf, 0x7f],
		&[0xdf, 0xc0],
		&[0xe0, 0xa0, 0x7f],
		&[0xe0, 0xa0, 0xc0],
		&[0xff],
	] {
		assert_eq!(decode_modified_utf8(invalid), None, "{invalid:?}");
	}
}
