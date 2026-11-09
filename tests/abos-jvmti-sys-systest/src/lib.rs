#[cfg(test)]
mod tests {
	use std::mem::{align_of, offset_of, size_of};

	use abos_jvmti_sys as sys;

	unsafe extern "C" {
		fn abos_jvmti_probe_count() -> usize;
		fn abos_jvmti_probe_value(index: usize) -> i64;
		fn abos_jvmti_get_capability(caps: *const sys::jvmtiCapabilities, index: usize) -> u32;
		fn abos_jvmti_set_capability(caps: *mut sys::jvmtiCapabilities, index: usize, value: u32);
	}

	#[test]
	fn rust_declarations_match_jdk_headers() {
		let expected = include!(concat!(env!("OUT_DIR"), "/expected.rs"));
		// SAFETY: These C functions only read a static array, with checked indices.
		assert_eq!(unsafe { abos_jvmti_probe_count() }, expected.len());
		for (index, (label, rust_value)) in expected.iter().enumerate() {
			// SAFETY: The count assertion bounds the index.
			assert_eq!(*rust_value, unsafe { abos_jvmti_probe_value(index) }, "{label}");
		}
		println!(
			"{} ABI measurements match jvmti.h and jvmticmlr.h; C signature checks passed",
			expected.len()
		);
	}

	#[test]
	fn every_capability_round_trips_between_c_and_rust() {
		type Getter = fn(&sys::jvmtiCapabilities) -> bool;
		type Setter = fn(&mut sys::jvmtiCapabilities, bool);
		let capabilities = include!(concat!(env!("OUT_DIR"), "/capabilities.rs"));
		for (index, (name, getter, setter)) in capabilities.iter().enumerate() {
			let mut from_c = sys::jvmtiCapabilities::default();
			// SAFETY: The capability record is writable and index selects a known bitfield.
			unsafe { abos_jvmti_set_capability(&raw mut from_c, index, 1) };
			assert!(getter(&from_c), "C -> Rust: {name}");
			let mut from_rust = sys::jvmtiCapabilities::default();
			setter(&mut from_rust, true);
			assert_eq!(from_c, from_rust, "bitfield storage: {name}");
			for (other, (other_name, other_getter, _)) in capabilities.iter().enumerate() {
				let expected = u32::from(index == other);
				assert_eq!(
					u32::from(other_getter(&from_c)),
					expected,
					"{name} affected {other_name}"
				);
				// SAFETY: The record is readable and other is a known bitfield index.
				assert_eq!(
					unsafe { abos_jvmti_get_capability(&raw const from_rust, other) },
					expected
				);
			}
			setter(&mut from_c, false);
			assert_eq!(from_c, sys::jvmtiCapabilities::default(), "clear {name}");
		}
	}
}
