#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	// This additionally parses member descriptors and builds binding metadata.
	let _ = abos_jbindgen::fuzzing::parse_class(data);
});
