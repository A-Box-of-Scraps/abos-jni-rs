#![no_main]

use std::ffi::CString;

use abos_jni::strings::{JNIStr, JNIString};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
	// CString supplies the terminator and rejects interior NULs without unsafe constructors.
	let Ok(cstring) = CString::new(data) else {
		return;
	};
	if let Some(validated) = JNIStr::from_cstr(&cstring) {
		assert_eq!(validated.to_bytes(), data);
		let decoded = validated.to_str();
		// Unpaired surrogate code units are accepted but decoded lossily. Only
		// the resulting Rust string, not the original bytes, must round-trip.
		assert_eq!(JNIString::new(&decoded).to_str(), decoded);
	}
});
