use core::{ffi::c_char, ops::Deref};

use abos_jni::strings::JNIStr;

use super::array::Deallocator;
use crate::{Env, Error, JvmtiArray, Result};

/// A JVMTI-owned, NUL-terminated modified UTF-8 string.
///
/// Dereferences to [`JNIStr`], not UTF-8 `str`. This is native allocated memory,
/// not a JNI reference or a `GetStringUTFChars` lease. The environment, thread,
/// phase, callback, and cleanup requirements of [`JvmtiArray`] apply.
///
/// A borrowed string cannot escape its allocation owner:
///
/// ```compile_fail
/// use abos_jni::strings::JNIStr;
/// use abos_jvmti::JvmtiString;
/// fn escape<'a>(owner: JvmtiString<'_, '_>) -> &'a JNIStr {
///     &owner
/// }
/// ```
#[derive(Debug)]
pub struct JvmtiString<'env, 'vm> {
	bytes: JvmtiArray<'env, 'vm, u8>,
}

impl<'env, 'vm> JvmtiString<'env, 'vm> {
	/// Takes ownership of a non-null JVMTI modified UTF-8 string.
	///
	/// On error ownership stays with the caller. A missing optional string must
	/// be represented separately, not passed as a null pointer here.
	///
	/// # Errors
	/// Returns a missing-deallocator, null-pointer, or size error.
	///
	/// # Safety
	/// The pointer must start an exclusively owned allocation from `env` containing
	/// a valid NUL-terminated modified UTF-8 string, including the terminator, within
	/// `isize::MAX` bytes. [`JvmtiArray::from_raw`]'s context and aliasing rules apply.
	pub unsafe fn from_raw(env: &'env Env<'vm>, pointer: *mut c_char) -> Result<Self> {
		if pointer.is_null() {
			return Err(Error::NullPointer);
		}
		// Check before reading the string, and before accepting ownership.
		let _deallocator: Deallocator<'_, '_> = Deallocator::new(env)?;
		// SAFETY: The caller guarantees a readable, terminated modified UTF-8 string.
		let length: usize = unsafe { JNIStr::from_ptr(pointer) }.to_bytes().len() + 1;
		// SAFETY: All bytes including the terminator are initialized and exclusively owned.
		let bytes: JvmtiArray<'_, '_, u8> = unsafe { JvmtiArray::from_raw(env, pointer.cast(), length) }?;
		Ok(Self { bytes })
	}

	/// Transfers the string allocation to the caller, who must use the originating
	/// environment's `Deallocate` or transfer it to an operation taking ownership.
	#[must_use]
	pub fn into_raw(self) -> *mut c_char {
		self.bytes.into_raw().0.cast()
	}

	/// Frees the string once, without retrying on failure.
	///
	/// # Errors
	/// Returns a native cleanup error. See [`JvmtiArray::free`] for failure policy.
	pub fn free(self) -> Result<()> {
		self.bytes.free()
	}
}

impl Deref for JvmtiString<'_, '_> {
	type Target = JNIStr;
	fn deref(&self) -> &JNIStr {
		// SAFETY: Construction requires valid MUTF-8 and no mutable bytes are exposed.
		unsafe { JNIStr::from_ptr(self.bytes.as_ptr().cast()) }
	}
}

/// An owned array of separately allocated modified UTF-8 strings.
///
/// Used by [`Env::system_properties`]. Each string is deallocated before the
/// outer array. [`JvmtiArray`]'s environment and cleanup requirements apply.
#[derive(Debug)]
pub struct JvmtiStringList<'env, 'vm> {
	strings: JvmtiArray<'env, 'vm, *mut c_char>,
}

impl<'env, 'vm> JvmtiStringList<'env, 'vm> {
	pub(crate) unsafe fn from_output(
		deallocator: Deallocator<'env, 'vm>,
		pointer: *mut *mut c_char,
		count: crate::sys::jint,
	) -> Result<Self> {
		// SAFETY: The successful operation returns distinct owned strings and their array.
		let strings: JvmtiArray<'_, '_, *mut c_char> =
			unsafe { JvmtiArray::from_output(deallocator, pointer, count, Some(free_strings)) }?;
		if strings.iter().any(|pointer| pointer.is_null()) {
			return Err(Error::NullPointer);
		}
		Ok(Self { strings })
	}

	/// Returns the number of strings.
	#[must_use]
	pub fn len(&self) -> usize {
		self.strings.len()
	}

	/// Returns whether the array is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.strings.is_empty()
	}

	/// Borrows the strings for at most the lifetime of this owner.
	pub fn iter(&self) -> impl ExactSizeIterator<Item = &JNIStr> + DoubleEndedIterator {
		self.strings.iter().map(|&pointer| {
			// SAFETY: Valid strings come from the VM; nulls are rejected before exposure.
			unsafe { JNIStr::from_ptr(pointer) }
		})
	}

	/// Transfers the outer pointer and count, including ownership of every string.
	/// Each string and then the array must be deallocated with the originating env.
	#[must_use]
	pub fn into_raw(self) -> (*mut *mut c_char, usize) {
		self.strings.into_raw()
	}

	/// Attempts to free every string and the outer array, even after an error.
	///
	/// # Errors
	/// Returns the first cleanup error. See [`JvmtiArray::free`] for failure policy.
	pub fn free(self) -> Result<()> {
		self.strings.free()
	}
}

unsafe fn free_strings(deallocator: Deallocator<'_, '_>, strings: &mut [*mut c_char]) -> Result<()> {
	let mut result: Result<()> = Ok(());
	for &mut pointer in strings {
		// SAFETY: Each non-null pointer is a distinct allocation owned by the list.
		let cleanup: Result<()> = unsafe { deallocator.free(pointer) };
		result = result.and(cleanup);
	}
	result
}
