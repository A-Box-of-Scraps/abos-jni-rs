use core::{
	mem::ManuallyDrop,
	ops::{Deref, DerefMut},
	ptr::NonNull,
};

use crate::{Env, Error, Result, internal::error::check, sys};

#[derive(Clone, Copy, Debug)]
pub struct Deallocator<'env, 'vm> {
	env: &'env Env<'vm>,
	function: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut u8) -> sys::jvmtiError,
}

impl<'env, 'vm> Deallocator<'env, 'vm> {
	pub(crate) fn new(env: &'env Env<'vm>) -> Result<Self> {
		Ok(Self {
			env,
			function: env.table().Deallocate.ok_or(Error::MissingFunction("Deallocate"))?,
		})
	}

	pub(crate) unsafe fn free<T>(self, pointer: *mut T) -> Result<()> {
		if pointer.is_null() {
			return Ok(());
		}
		// SAFETY: The caller supplies a live allocation from this environment.
		check(unsafe { (self.function)(self.env.as_raw(), pointer.cast()) })
	}
}

type Cleanup<T> = unsafe fn(Deallocator<'_, '_>, &mut [T]) -> Result<()>;

/// An initialized array owned by the originating JVMTI environment.
///
/// Only the outer allocation is owned by [`Self::from_raw`]. Elements are `Copy`
/// and are never Rust-dropped. In particular, JNI references must be managed
/// separately with JNI: this handle neither deletes references nor extends their
/// local-frame, thread, or VM lifetimes. Pointer elements do not imply ownership
/// of their pointees. Use operation-specific owners for nested allocations.
///
/// This owner is neither `Send` nor `Sync`. The environment and VM must remain
/// alive through access and cleanup. Access to the native slice makes no VM call;
/// interpreting embedded JNI handles still requires the appropriate JNI context.
/// Cleanup requires a JVMTI-permitted attached thread (or an explicitly permitted
/// startup/callback context). `Deallocate` is allowed in all phases and is
/// callback-safe, including heap, GC, and `ObjectFree` callbacks.
///
/// Drop attempts cleanup once and ignores errors. [`Self::free`] reports errors;
/// [`Self::into_raw`] transfers responsibility to the caller instead.
///
/// The owner cannot outlive the environment handle it borrows:
///
/// ```compile_fail
/// use abos_jvmti::{Env, JvmtiArray};
/// fn escape<'vm>(env: Env<'vm>) -> JvmtiArray<'static, 'vm, u8> {
///     unsafe { JvmtiArray::from_raw(&env, core::ptr::null_mut(), 0).unwrap() }
/// }
/// ```
#[derive(Debug)]
pub struct JvmtiArray<'env, 'vm, T: Copy> {
	deallocator: Deallocator<'env, 'vm>,
	pointer: *mut T,
	length: usize,
	cleanup: Option<Cleanup<T>>,
}

impl<'env, 'vm, T: Copy> JvmtiArray<'env, 'vm, T> {
	/// Adopts an initialized, flat JVMTI allocation, without copying it.
	///
	/// A null pointer is accepted only for length zero. A non-null pointer with
	/// length zero is retained and deallocated. On error ownership stays with the
	/// caller. No Rust allocator or element destructor is used.
	///
	/// # Errors
	/// Returns a missing-deallocator, null-pointer, or size error.
	///
	/// # Safety
	/// A non-null pointer must be the start of an exclusively owned allocation
	/// returned by this environment, aligned for `T`, containing `length` valid,
	/// initialized elements. The entire slice must lie within that allocation.
	/// Do not adopt callback-borrowed data, interior pointers, or JNI string/array
	/// elements. All aliases must obey Rust borrowing rules. The environment, VM,
	/// and cleanup context must satisfy the requirements in [`JvmtiArray`].
	pub unsafe fn from_raw(env: &'env Env<'vm>, pointer: *mut T, length: usize) -> Result<Self> {
		let deallocator: Deallocator<'_, '_> = Deallocator::new(env)?;
		Self::new(deallocator, pointer, length, None)
	}

	fn new(
		deallocator: Deallocator<'env, 'vm>,
		pointer: *mut T,
		length: usize,
		cleanup: Option<Cleanup<T>>,
	) -> Result<Self> {
		if length > (isize::MAX as usize) / size_of::<T>().max(1) {
			return Err(Error::AllocationTooLarge);
		}
		if pointer.is_null() && length != 0 {
			return Err(Error::NullPointer);
		}
		Ok(Self {
			deallocator,
			pointer,
			length,
			cleanup,
		})
	}

	/// Adopts outputs only after a successful native call. Invalid counts cannot
	/// describe children, but the independently owned outer allocation is freed.
	pub(crate) unsafe fn from_output(
		deallocator: Deallocator<'env, 'vm>,
		pointer: *mut T,
		count: sys::jint,
		cleanup: Option<Cleanup<T>>,
	) -> Result<Self> {
		let result: Result<Self> = usize::try_from(count)
			.map_err(|_| Error::InvalidCount(count))
			.and_then(|length| Self::new(deallocator, pointer, length, cleanup));
		if result.is_err() {
			// SAFETY: Successful JVMTI outputs own the outer allocation, even if its count is invalid.
			let _cleanup: Result<()> = unsafe { deallocator.free(pointer) };
		}
		result
	}

	/// Transfers the original pointer and element count without freeing anything.
	///
	/// The caller must use this environment's `Deallocate`, not Rust's allocator.
	/// Null and non-null empty pointers are preserved. Embedded JNI references
	/// remain independently managed.
	#[must_use]
	pub fn into_raw(self) -> (*mut T, usize) {
		let owner: ManuallyDrop<Self> = ManuallyDrop::new(self);
		(owner.pointer, owner.length)
	}

	/// Consumes the owner and attempts every owned deallocation once.
	///
	/// # Errors
	/// Reports the first native cleanup error. Cleanup continues after errors and
	/// drop does not retry. Failed deallocations may leak; use [`Self::into_raw`]
	/// instead if the caller needs to retain pointers for its own recovery policy.
	pub fn free(mut self) -> Result<()> {
		self.release()
	}

	fn release(&mut self) -> Result<()> {
		let deallocator: Deallocator<'_, '_> = self.deallocator;
		let nested: Result<()> = self.cleanup.take().map_or(Ok(()), |cleanup| {
			// SAFETY: Only operation-specific constructors install cleanup for their owned children.
			unsafe { cleanup(deallocator, self) }
		});
		let pointer: *mut T = core::mem::replace(&mut self.pointer, core::ptr::null_mut());
		self.length = 0;
		// SAFETY: Ownership is removed before calling Deallocate, preventing drop retries.
		let outer: Result<()> = unsafe { deallocator.free(pointer) };
		nested.and(outer)
	}
}

impl<T: Copy> Deref for JvmtiArray<'_, '_, T> {
	type Target = [T];
	fn deref(&self) -> &[T] {
		let pointer: *mut T = if self.length == 0 {
			NonNull::dangling().as_ptr()
		} else {
			self.pointer
		};
		// SAFETY: Construction checks slice bounds and requires initialized aligned storage.
		unsafe { core::slice::from_raw_parts(pointer, self.length) }
	}
}

impl<T: Copy> DerefMut for JvmtiArray<'_, '_, T> {
	fn deref_mut(&mut self) -> &mut [T] {
		let pointer: *mut T = if self.length == 0 {
			NonNull::dangling().as_ptr()
		} else {
			self.pointer
		};
		// SAFETY: The owner is exclusive and &mut self prevents overlapping borrows.
		unsafe { core::slice::from_raw_parts_mut(pointer, self.length) }
	}
}

impl<T: Copy> Drop for JvmtiArray<'_, '_, T> {
	fn drop(&mut self) {
		let _cleanup: Result<()> = self.release();
	}
}
