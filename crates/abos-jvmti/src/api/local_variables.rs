use abos_jni::strings::JNIStr;

use super::array::Deallocator;
use crate::{Error, JvmtiArray, Result, sys};

/// A borrowed local-variable description; all strings borrow the owning table.
#[derive(Clone, Copy, Debug)]
pub struct LocalVariable<'table> {
	/// First code-array index where the variable is valid.
	pub start_location: sys::jlocation,
	/// Length of the variable's valid code range.
	pub length: sys::jint,
	/// Modified UTF-8 variable name.
	pub name: &'table JNIStr,
	/// Modified UTF-8 type signature.
	pub signature: &'table JNIStr,
	/// Optional modified UTF-8 generic signature.
	pub generic_signature: Option<&'table JNIStr>,
	/// Local-variable slot number.
	pub slot: sys::jint,
}

/// Owns a `GetLocalVariableTable` result and all of its nested string allocations.
///
/// Cleanup frees names, signatures, and non-null generic signatures before the
/// table itself. No JNI references are owned. [`JvmtiArray`]'s environment,
/// thread-attachment, phase, and callback-context requirements apply.
#[derive(Debug)]
pub struct LocalVariableTable<'env, 'vm> {
	entries: JvmtiArray<'env, 'vm, sys::jvmtiLocalVariableEntry>,
}

impl<'env, 'vm> LocalVariableTable<'env, 'vm> {
	pub(crate) unsafe fn from_output(
		deallocator: Deallocator<'env, 'vm>,
		pointer: *mut sys::jvmtiLocalVariableEntry,
		count: sys::jint,
	) -> Result<Self> {
		// SAFETY: The successful operation supplies initialized entries and distinct nested allocations.
		let entries: JvmtiArray<'_, '_, sys::jvmtiLocalVariableEntry> =
			unsafe { JvmtiArray::from_output(deallocator, pointer, count, Some(free_entries)) }?;
		if entries
			.iter()
			.any(|entry| entry.name.is_null() || entry.signature.is_null())
		{
			return Err(Error::NullPointer);
		}
		Ok(Self { entries })
	}

	/// Returns the number of entries.
	#[must_use]
	pub fn len(&self) -> usize {
		self.entries.len()
	}

	/// Returns whether the table is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Borrows entries and their strings without exposing mutable owning pointers.
	pub fn iter(&self) -> impl ExactSizeIterator<Item = LocalVariable<'_>> + DoubleEndedIterator {
		self.entries.iter().map(|entry| {
			// SAFETY: Construction validates required pointers; VM strings are valid MUTF-8
			// and live as long as the table. Generic signatures may legitimately be null.
			unsafe {
				LocalVariable {
					start_location: entry.start_location,
					length: entry.length,
					name: JNIStr::from_ptr(entry.name),
					signature: JNIStr::from_ptr(entry.signature),
					generic_signature: if entry.generic_signature.is_null() {
						None
					} else {
						Some(JNIStr::from_ptr(entry.generic_signature))
					},
					slot: entry.slot,
				}
			}
		})
	}

	/// Transfers the table and all nested allocations to the caller.
	///
	/// Deallocate every non-null string field, then the table, using the originating
	/// environment. The returned count is the number of entries, not bytes.
	#[must_use]
	pub fn into_raw(self) -> (*mut sys::jvmtiLocalVariableEntry, usize) {
		self.entries.into_raw()
	}

	/// Attempts every nested deallocation and then the table, even after errors.
	///
	/// # Errors
	/// Returns the first cleanup error. See [`JvmtiArray::free`] for failure policy.
	pub fn free(self) -> Result<()> {
		self.entries.free()
	}
}

unsafe fn free_entries(deallocator: Deallocator<'_, '_>, entries: &mut [sys::jvmtiLocalVariableEntry]) -> Result<()> {
	let mut result: Result<()> = Ok(());
	for entry in entries {
		for pointer in [entry.name, entry.signature, entry.generic_signature] {
			// SAFETY: Non-null string fields are distinct allocations owned by this table.
			let cleanup: Result<()> = unsafe { deallocator.free(pointer) };
			result = result.and(cleanup);
		}
	}
	result
}
