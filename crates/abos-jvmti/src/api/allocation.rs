use core::ops::{Deref, DerefMut};

use super::array::Deallocator;
use crate::{Env, Error, JvmtiArray, Result, internal::error::check, sys};

/// A zero-initialized byte allocation owned by a JVMTI environment.
///
/// Drop releases it with the originating environment's `Deallocate`, never
/// Rust's allocator. [`JvmtiArray`]'s lifetime and cleanup-context requirements
/// apply. A native deallocation failure during drop cannot be reported; use
/// [`Self::free`] to observe cleanup errors.
#[derive(Debug)]
pub struct Allocation<'env, 'vm> {
	bytes: JvmtiArray<'env, 'vm, u8>,
}

impl<'vm> Env<'vm> {
	/// Allocates zero-initialized bytes with the JVMTI allocator.
	///
	/// # Errors
	/// Returns a size, native allocation, missing-function, or null-pointer error.
	///
	/// # Safety
	/// The current thread must satisfy JVMTI's attachment requirements now and
	/// when the allocation is dropped. Allocation and deallocation are callback-safe
	/// in all phases. Keep the environment and VM alive until the allocation is freed
	/// or its ownership is transferred to the VM.
	pub unsafe fn allocate(&self, length: usize) -> Result<Allocation<'_, 'vm>> {
		let size: sys::jlong = length.try_into().map_err(|_| Error::AllocationTooLarge)?;
		if length > isize::MAX as usize {
			return Err(Error::AllocationTooLarge);
		}
		let allocate: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jlong, *mut *mut u8) -> sys::jvmtiError =
			self.table().Allocate.ok_or(Error::MissingFunction("Allocate"))?;
		let _deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut raw: *mut u8 = core::ptr::null_mut();
		// SAFETY: The caller guarantees the context; size is representable and raw writable.
		check(unsafe { allocate(self.as_raw(), size, &raw mut raw) })?;
		if length != 0 {
			if raw.is_null() {
				return Err(Error::NullPointer);
			}
			// SAFETY: Allocate returned exclusive writable storage for length bytes.
			unsafe { raw.write_bytes(0, length) };
		}
		// SAFETY: Storage is initialized; null is permitted only for an empty allocation.
		let bytes: JvmtiArray<'_, '_, u8> = unsafe { JvmtiArray::from_raw(self, raw, length) }?;
		Ok(Allocation { bytes })
	}
}

impl Allocation<'_, '_> {
	/// Transfers ownership to the caller, for example for a class-file load hook.
	///
	/// The result must be released with the same environment's `Deallocate` or
	/// transferred to a JVMTI operation that takes ownership. The original pointer
	/// is preserved, including null for a zero-sized JVMTI allocation.
	#[must_use]
	pub fn into_raw(self) -> *mut u8 {
		self.bytes.into_raw().0
	}

	/// Consumes the allocation and attempts cleanup once, without a drop retry.
	///
	/// # Errors
	/// Returns a native cleanup error. See [`JvmtiArray::free`] for failure policy.
	pub fn free(self) -> Result<()> {
		self.bytes.free()
	}
}

impl Deref for Allocation<'_, '_> {
	type Target = [u8];
	fn deref(&self) -> &[u8] {
		&self.bytes
	}
}

impl DerefMut for Allocation<'_, '_> {
	fn deref_mut(&mut self) -> &mut [u8] {
		&mut self.bytes
	}
}
