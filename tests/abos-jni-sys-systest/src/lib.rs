#[cfg(test)]
mod tests {
	use std::mem::{align_of, offset_of, size_of};

	use abos_jni_sys as sys;

	unsafe extern "C" {
		fn abos_jni_probe_count() -> usize;
		fn abos_jni_probe_value(index: usize) -> i64;
	}

	#[test]
	fn rust_bindings_match_jdk_headers() {
		let expected = include!(concat!(env!("OUT_DIR"), "/expected.rs"));
		// Both functions access only a static C array. The count check ensures each
		// subsequent index is valid; no JVM pointers or function calls are involved.
		assert_eq!(unsafe { abos_jni_probe_count() }, expected.len());
		for (index, (label, rust_value)) in expected.iter().enumerate() {
			let c_value = unsafe { abos_jni_probe_value(index) };
			assert_eq!(*rust_value, c_value, "{label}: Rust differs from jni.h");
		}
		println!(
			"{} ABI measurements match jni.h; C type/signature assertions passed at build time",
			expected.len()
		);
	}
}
