#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	// Rejecting malformed or unsupported class files is expected, not a failure.
	let _ = abos_cafebabe::parse_class(data);
});
