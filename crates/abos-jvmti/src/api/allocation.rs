use core::{
	mem::ManuallyDrop,
	ops::{Deref, DerefMut},
	ptr::NonNull,
};

use crate::{Env, Error, Result, internal::error::check, sys};

/// A zero-initialized byte allocation owned by a JVMTI environment.
///
/// Drop releases it with JVMTI `Deallocate`, never Rust's allocator. It cannot
/// move between threads. The environment and VM must remain alive through drop;
/// the creating thread must remain in a context where `Deallocate` is permitted.
/// A native deallocation failure during drop cannot be reported.
#[derive(Debug)]
pub struct Allocation<'env, 'vm> {
	env: &'env Env<'vm>,
	pointer: NonNull<u8>,
	length: usize,
	deallocate: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut u8) -> sys::jvmtiError,
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
		let deallocate: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut u8) -> sys::jvmtiError =
			self.table().Deallocate.ok_or(Error::MissingFunction("Deallocate"))?;
		let mut raw: *mut u8 = core::ptr::null_mut();
		// SAFETY: The caller guarantees the context; size is representable and raw writable.
		check(unsafe { allocate(self.as_raw(), size, &raw mut raw) })?;
		let pointer: NonNull<u8> = if length == 0 {
			// JVMTI specifies a null allocation for size zero. Use a dangling pointer
			// solely to satisfy the non-null requirement of empty Rust slices.
			NonNull::dangling()
		} else {
			let pointer: NonNull<u8> = NonNull::new(raw).ok_or(Error::NullPointer)?;
			// SAFETY: Allocate returned exclusive writable storage for length bytes.
			unsafe { pointer.as_ptr().write_bytes(0, length) };
			pointer
		};
		Ok(Allocation {
			env: self,
			pointer,
			length,
			deallocate,
		})
	}
}

impl Allocation<'_, '_> {
	/// Transfers ownership to the caller, for example for a class-file load hook.
	///
	/// The result must be released with the same environment's `Deallocate` or
	/// transferred to a JVMTI operation that takes ownership. Returns null for an
	/// empty allocation. Dropping the Rust handle no longer frees the memory.
	#[must_use]
	pub fn into_raw(self) -> *mut u8 {
		let allocation: ManuallyDrop<Self> = ManuallyDrop::new(self);
		if allocation.length == 0 {
			core::ptr::null_mut()
		} else {
			allocation.pointer.as_ptr()
		}
	}
}

impl Deref for Allocation<'_, '_> {
	type Target = [u8];

	fn deref(&self) -> &[u8] {
		// SAFETY: The allocation is initialized, valid, and bounded by isize::MAX;
		// the empty case uses a non-null aligned dangling pointer.
		unsafe { core::slice::from_raw_parts(self.pointer.as_ptr(), self.length) }
	}
}

impl DerefMut for Allocation<'_, '_> {
	fn deref_mut(&mut self) -> &mut [u8] {
		// SAFETY: Ownership and the mutable borrow guarantee exclusive access.
		unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.length) }
	}
}

impl Drop for Allocation<'_, '_> {
	fn drop(&mut self) {
		if self.length != 0 {
			// SAFETY: The allocation is still owned here. The allocate contract requires
			// its environment and thread context to remain suitable through drop.
			let _code: sys::jvmtiError = unsafe { (self.deallocate)(self.env.as_raw(), self.pointer.as_ptr()) };
		}
	}
}
