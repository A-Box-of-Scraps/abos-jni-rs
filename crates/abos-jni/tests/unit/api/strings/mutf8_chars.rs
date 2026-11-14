use std::{borrow::Cow, ffi::CStr, mem::ManuallyDrop};

use super::{JNIStr, JNIString, JString, MUTF8Chars, OnceCell, PhantomData};

// These tests exercise only the character view and ownership transfer, not JNI.
// Suppress Drop because the synthetic buffers do not belong to a JVM.
fn synthetic_chars(input: &'static CStr, is_copy: bool) -> ManuallyDrop<MUTF8Chars<'static, JString<'static>>> {
	ManuallyDrop::new(MUTF8Chars {
		obj: JString::null(),
		chars: input.as_ptr(),
		normalized: OnceCell::new(),
		is_copy,
		_lifetime: PhantomData,
	})
}

#[test]
fn normalized_view_is_cached_but_raw_transfer_keeps_the_jvm_pointer() {
	let original: &CStr = c"\xC0\x80😀\xED\xA0\x80";
	for is_copy in [false, true] {
		let chars: ManuallyDrop<MUTF8Chars<'_, JString<'_>>> = synthetic_chars(original, is_copy);
		let view: &JNIStr = (&*chars).into();
		let expected: &JNIStr =
			JNIStr::from_cstr(c"\xC0\x80\xED\xA0\xBD\xED\xB8\x80\xED\xA0\x80").expect("strict MUTF-8");
		assert_eq!(view, expected);
		assert!(chars.normalized.get().expect("cache initialized").is_some());
		assert_ne!(view.as_ptr(), original.as_ptr());
		assert_eq!(view.as_ptr(), chars.as_ptr());
		assert_eq!(chars.is_copy(), is_copy);
		assert_eq!(chars.to_string(), "\0😀\u{FFFD}");
		let cow: Cow<'_, str> = (&*chars).into();
		assert_eq!(cow, "\0😀\u{FFFD}");
		let owned: JNIString = view.to_owned();
		assert_eq!(&*owned, expected);

		// into_raw drops the normalization cache but must not release or replace
		// the original pointer. It never accesses the synthetic object reference.
		let (pointer, copied): (*const std::ffi::c_char, bool) = ManuallyDrop::into_inner(chars).into_raw();
		assert_eq!(pointer, original.as_ptr());
		assert_eq!(copied, is_copy);
	}
}

#[test]
fn strict_view_borrows_the_original_buffer() {
	let original: &CStr = c"borrowed\xC0\x80\xED\xA0\xBD\xED\xB8\x80";
	let chars: ManuallyDrop<MUTF8Chars<'_, JString<'_>>> = synthetic_chars(original, false);
	assert_eq!(chars.as_ptr(), original.as_ptr());
	assert!(chars.normalized.get().expect("cache initialized").is_none());
	assert_eq!(chars.to_str(), "borrowed\0😀");
	let (pointer, _): (*const std::ffi::c_char, bool) = ManuallyDrop::into_inner(chars).into_raw();
	assert_eq!(pointer, original.as_ptr());
}

#[test]
fn raw_transfer_before_normalization_does_not_allocate() {
	let original: &CStr = c"\xC0\x80😀";
	let chars: ManuallyDrop<MUTF8Chars<'_, JString<'_>>> = synthetic_chars(original, true);
	assert!(chars.normalized.get().is_none());
	let (pointer, copied): (*const std::ffi::c_char, bool) = ManuallyDrop::into_inner(chars).into_raw();
	assert_eq!(pointer, original.as_ptr());
	assert!(copied);
}
