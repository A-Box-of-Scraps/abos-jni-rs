#![no_main]

use abos_jni::signature::{JavaType, RuntimeFieldSignature, RuntimeMethodSignature};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	let Ok(input) = std::str::from_utf8(data) else {
		return;
	};
	let _ = input.parse::<JavaType>();
	if let Ok(parsed) = RuntimeFieldSignature::from_str(input) {
		assert_eq!(parsed.field_signature().sig().to_str(), input);
	}
	if let Ok(parsed) = RuntimeMethodSignature::from_str(input) {
		assert_eq!(parsed.method_signature().sig().to_str(), input);
	}
});
