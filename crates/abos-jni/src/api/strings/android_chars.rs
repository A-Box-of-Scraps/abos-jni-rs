use std::ffi::{CStr, CString};

use super::JNIString;

/// Converts ART's four-byte UTF-8 sequences to MUTF-8 without changing any
/// existing MUTF-8 code units, including encoded NULs and unpaired surrogates.
/// Returns `None` when the original buffer already uses strict MUTF-8.
///
/// # Safety
/// `chars` must contain valid MUTF-8 code units, optionally mixed with valid
/// four-byte UTF-8 sequences, as returned by ART's `GetStringUTFChars`.
pub(super) unsafe fn normalize(chars: &CStr) -> Option<JNIString> {
	let mut remaining: &[u8] = chars.to_bytes();
	let mut next: usize = remaining.iter().position(|&byte| byte >= 0xF0)?;
	let mut encoded: Vec<u8> = Vec::with_capacity(remaining.len());

	loop {
		encoded.extend_from_slice(&remaining[..next]);
		let (sequence, rest): (&[u8], &[u8]) = remaining[next..].split_at(4);
		let character: char = std::str::from_utf8(sequence)
			.expect("ART returns valid four-byte UTF-8 sequences")
			.chars()
			.next()
			.expect("a four-byte sequence contains one character");

		// MUTF-8 encodes each half of a surrogate pair as a three-byte unit.
		for unit in character.encode_utf16(&mut [0; 2]) {
			let [high, low]: [u8; 2] = unit.to_be_bytes();
			encoded.extend_from_slice(&[
				0xE0 | (high >> 4),
				0x80 | ((high & 0x0F) << 2) | (low >> 6),
				0x80 | (low & 0x3F),
			]);
		}

		remaining = rest;
		let Some(index) = remaining.iter().position(|&byte| byte >= 0xF0) else {
			break;
		};
		next = index;
	}
	encoded.extend_from_slice(remaining);

	let encoded: CString = CString::new(encoded).expect("MUTF-8 contains no interior NUL bytes");
	// SAFETY: Existing MUTF-8 units were retained verbatim; each four-byte sequence
	// was replaced by its two MUTF-8 surrogate units, and CString adds the terminator.
	Some(unsafe { JNIString::from_cstring(encoded) })
}

#[cfg(test)]
#[path = "../../../tests/unit/api/strings/android_chars.rs"]
mod tests;
