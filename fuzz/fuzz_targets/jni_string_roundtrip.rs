#![no_main]

use abos_jni::strings::{JNIStr, JNIString};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	let Ok(input) = std::str::from_utf8(data) else {
		return;
	};
	let encoded = JNIString::new(input);
	assert_eq!(encoded.to_str(), input);
	let validated = JNIStr::from_cstr(encoded.as_cstr()).expect("encoder must produce valid modified UTF-8");
	assert_eq!(validated.to_str(), input);
});
