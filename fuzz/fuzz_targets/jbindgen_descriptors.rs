#![no_main]

use abos_jbindgen::fuzzing::{parse_method_descriptor, parse_type_descriptor};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	let Ok(input) = std::str::from_utf8(data) else {
		return;
	};
	// These parsers have different validation policies from abos-jni; do not
	// assume that both crates must accept exactly the same descriptors.
	let _ = parse_type_descriptor(input);
	let _ = parse_method_descriptor(input);
});
