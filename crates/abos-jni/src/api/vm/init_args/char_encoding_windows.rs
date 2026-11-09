use super::{
	JvmError,
	char_encoding_generic::{bytes_to_cstr, utf8_to_cstr},
};
use crate::windows_sys;
use std::{
	borrow::Cow,
	convert::TryInto,
	ffi::{CStr, c_int, c_uint},
	io,
	mem::MaybeUninit,
	ptr,
};

// The integer type used by `WideCharToMultiByte` for string lengths.
type WSize = c_int;

// The type of Windows codepage numbers.
type WCodepage = c_uint;

// Win32 accepts unsigned code page IDs, but the metadata constants are signed.
const CP_UTF7: WCodepage = windows_sys::CP_UTF7.cast_unsigned();
const CP_UTF8: WCodepage = windows_sys::CP_UTF8.cast_unsigned();

// The maximum length, in UTF-8 bytes, of strings that will be accepted for transcoding.
//
// The purpose of this limit is to prevent overflow. `WideCharToMultiByte` behaves rather badly
// (see https://github.com/jni-rs/jni-rs/pull/414 for discussion) if the string is long enough to
// overflow its counters.
//
// Although it is possible to transcode a string of any length by splitting it into smaller
// substrings, the code complexity needed to do so isn't worthwhile just for transcoding JVM
// options. Also, `test_overflow` would take a very long time to run, which was deemed unacceptable
// (see https://github.com/jni-rs/jni-rs/pull/414#issuecomment-1419130483). We set this arbitrary
// limit instead.
const MAX_INPUT_LEN: usize = 1_048_576;

/// Converts `s` into a `Cow<CStr>` encoded in the specified Windows code page.
pub(super) fn str_to_cstr_win32(s: Cow<str>, needed_codepage: WCodepage) -> Result<Cow<'static, CStr>, JvmError> {
	// This function will generate an error if `WideCharToMultiByte` fails.
	fn convert_error(s: Cow<str>) -> JvmError {
		JvmError::OptStringTranscodeFailure {
			opt_string: s.into_owned(),
			error: io::Error::last_os_error(),
		}
	}

	// First, check if the input string (UTF-8) is too long to transcode. Bail early if so.
	if s.len() > MAX_INPUT_LEN {
		return Err(JvmError::OptStringTooLong {
			opt_string: s.into_owned(),
		});
	}

	// Convert the string to UTF-16 first.
	let s_utf16: Vec<u16> = s.encode_utf16().collect();

	// Determine how long the string is, in UTF-16 units, in the integer type that Win32 expects.
	// Overflow should be impossible; panic if it happens.
	let s_utf16_len: WSize = s_utf16
		.len()
		.try_into()
		.expect("UTF-16 form of input string is too long");

	// Decide which flags we're going to use.
	let conversion_flags: u32 = match needed_codepage {
		// No flags may be given for the following code pages.
		// https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte
		42 | 50220 | 50221 | 50222 | 50225 | 50227 | 50229 | 54936 | 57002..=57011 | 65000 | 65001 => 0,

		_ => (windows_sys::WC_COMPOSITECHECK | windows_sys::WC_NO_BEST_FIT_CHARS).cast_unsigned(),
	};

	// Find out how much buffer space will be needed for the output and whether the string is
	// fully representable.
	let mut is_non_representable: Option<MaybeUninit<i32>> = match needed_codepage {
		// All characters are representable in UTF-7 and UTF-8, and moreover
		// `WideCharToMultiByte` will fail if the target encoding is UTF-7 or UTF-8 and this is not
		// `None`.
		CP_UTF7 | CP_UTF8 => None,
		_ => Some(MaybeUninit::uninit()),
	};

	// Safety: `s_utf16.as_ptr()` is a valid pointer to a UTF-16 string, and `s_utf16_len` is its
	// length. `lpDefaultChar` is null. `lpUsedDefaultChar` is either null or valid. `cbMultiByte`
	// is zero.
	let required_buffer_space: i32 = unsafe {
		windows_sys::WideCharToMultiByte(
			needed_codepage,
			conversion_flags,
			s_utf16.as_ptr(),
			s_utf16_len,
			ptr::null_mut(),
			0,
			ptr::null(),
			is_non_representable
				.as_mut()
				.map_or(ptr::null_mut(), MaybeUninit::as_mut_ptr),
		)
	};

	// Bail on error.
	if required_buffer_space == 0 {
		drop(s_utf16);

		return Err(convert_error(s));
	}

	// Check if the string is not fully representable.
	if let Some(is_non_representable) = is_non_representable {
		// Safety: `is_non_representable` has been initialized by `WideCharToMultiByte`.
		let is_non_representable: i32 = unsafe { is_non_representable.assume_init() };

		if is_non_representable != 0 {
			drop(s_utf16);

			return Err(JvmError::OptStringNotRepresentable {
				opt_string: s.into_owned(),
			});
		}
	}

	// Convert the required buffer space to `usize`, and increment it by one for the null
	// terminator.
	//
	// This shouldn't overflow (see the comment on `MAX_INPUT_LEN` above), so we won't check for
	// overflow here.
	let required_buffer_space_usize: usize =
		usize::try_from(required_buffer_space).expect("successful Win32 buffer size is positive");
	let required_buffer_space_usize_with_nul: usize = required_buffer_space_usize + 1;

	// Allocate enough buffer space, including one byte for the null terminator.
	let mut output: Vec<u8> = Vec::<u8>::with_capacity(required_buffer_space_usize_with_nul);

	// Perform the actual conversion.
	//
	// Safety: `s_utf16` holds `s_utf16_len` initialized UTF-16 units.
	// `output` has capacity for at least `required_buffer_space` bytes.
	// All other raw pointers are null.
	let used_buffer_space: i32 = unsafe {
		windows_sys::WideCharToMultiByte(
			needed_codepage,
			conversion_flags,
			s_utf16.as_ptr(),
			s_utf16_len,
			output.as_mut_ptr(),
			required_buffer_space,
			ptr::null(),
			ptr::null_mut(),
		)
	};

	drop(s_utf16);

	// Bail on error.
	if used_buffer_space == 0 {
		drop(output);

		return Err(convert_error(s));
	}

	let used_buffer_space_usize: usize =
		usize::try_from(used_buffer_space).expect("successful Win32 output length is positive");

	// Set the new length of the output buffer. Don't use `required_buffer_space`, just in case
	// `WideCharToMultiByte` changes its mind about how much buffer space it's actually going to
	// use.
	//
	// Safety: `used_buffer_space_usize` is the number of bytes that `WideCharToMultiByte` has
	// just initialized.
	unsafe {
		output.set_len(used_buffer_space_usize);
	}

	// That's it, it's converted. Now turn it into a `CString`. This will add a null terminator if
	// there isn't one already and check for null bytes in the middle.
	// SAFETY: The original UTF-8 string is supplied, so the output need not be valid UTF-8.
	unsafe { bytes_to_cstr(Cow::Owned(output), Some(s)) }
}

/// Converts `s` into the Windows default character encoding.
pub(super) fn str_to_cstr_win32_default_codepage(s: Cow<str>) -> Result<Cow<CStr>, JvmError> {
	// Get the code page. There is a remote possibility that it is UTF-8. If so, pass the
	// string through unchanged (other than adding a null terminator). If not, we need to have
	// Windows convert the string to the expected code page first.

	// Safety: This function isn't actually unsafe.
	let needed_codepage: u32 = unsafe { windows_sys::GetACP() };

	if needed_codepage == CP_UTF8 {
		// The code page is UTF-8! Lucky us.
		return utf8_to_cstr(s);
	}

	// The code page is not UTF-8, so do the transcoding.
	str_to_cstr_win32(s, needed_codepage)
}

#[cfg(test)]
#[path = "../../../../tests/unit/api/vm/init_args/char_encoding_windows.rs"]
mod tests;
