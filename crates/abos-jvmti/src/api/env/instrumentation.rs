use core::ptr;

use crate::{Env, Error, JvmtiArray, Result, api::array::Deallocator, internal::error::check, sys};

/// A borrowed class-file replacement for [`Env::redefine_classes`].
///
/// Neither the class reference nor the byte slice is retained by the VM after
/// redefinition returns. JNI reference validity is the caller's responsibility.
#[derive(Clone, Copy, Debug)]
pub struct ClassDefinition<'bytes> {
	/// The class to redefine, as a live JNI reference.
	pub class: sys::jclass,
	/// Complete class-file bytes, not just a method's bytecodes.
	pub bytes: &'bytes [u8],
}

impl Env<'_> {
	/// Retransforms classes using registered class-file hooks (JVMTI 1.2).
	///
	/// Requires `can_retransform_classes`, acquired before first enabling
	/// `ClassFileLoadHook`. No input ownership is transferred. This can synchronously
	/// invoke class-file hooks; active frames continue running their old bytecodes.
	///
	/// # Errors
	/// Returns a native error (including unmodifiable classes or missing capability),
	/// missing function, or an input length that cannot fit `jint`.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside restricted
	/// callbacks (not callback-safe). Every class reference must be valid for this
	/// thread and VM. Hooks must obey their callback contracts and must not unwind.
	pub unsafe fn retransform_classes(&self, classes: &[sys::jclass]) -> Result<()> {
		let count: sys::jint = super::management::count(classes.len())?;
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jint, *const sys::jclass) -> sys::jvmtiError =
			self.table()
				.RetransformClasses
				.ok_or(Error::MissingFunction("RetransformClasses"))?;
		// SAFETY: The slice is readable through the call; the caller supplies valid references and context.
		check(unsafe { function(self.as_raw(), count, classes.as_ptr()) })
	}

	/// Redefines classes from borrowed class-file bytes (JVMTI 1.2).
	///
	/// Requires `can_redefine_classes`; the VM validates class-file format and
	/// permitted structural changes. Inputs are not retained. Class-file hooks may
	/// run synchronously; active frames continue executing the previous definition.
	///
	/// # Errors
	/// Returns native validation/capability errors, a missing function, or an
	/// unrepresentable class count or byte length.
	///
	/// # Safety
	/// The phase, thread, callback, and reference requirements of
	/// [`Self::retransform_classes`] apply to every replacement class.
	pub unsafe fn redefine_classes(&self, definitions: &[ClassDefinition<'_>]) -> Result<()> {
		let count: sys::jint = super::management::count(definitions.len())?;
		let raw: Vec<sys::jvmtiClassDefinition> = definitions
			.iter()
			.map(|definition| {
				Ok(sys::jvmtiClassDefinition {
					klass: definition.class,
					class_byte_count: super::management::count(definition.bytes.len())?,
					class_bytes: definition.bytes.as_ptr(),
				})
			})
			.collect::<Result<_>>()?;
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jint,
			*const sys::jvmtiClassDefinition,
		) -> sys::jvmtiError = self
			.table()
			.RedefineClasses
			.ok_or(Error::MissingFunction("RedefineClasses"))?;
		// SAFETY: All class-file slices and the descriptor array remain readable until return.
		check(unsafe { function(self.as_raw(), count, raw.as_ptr()) })
	}

	/// Returns owned method bytecodes (JVMTI 1.2; `can_get_bytecodes`).
	///
	/// These are bytecodes only, not a complete class file for redefinition.
	///
	/// # Errors
	/// Returns native errors (including native methods or missing capability), a
	/// missing function, or an invalid output error.
	///
	/// # Safety
	/// Call in the start or live phase on a JVMTI-permitted attached thread,
	/// outside restricted callbacks (not callback-safe). The method ID must be
	/// valid. [`JvmtiArray`]'s environment and cleanup requirements apply.
	pub unsafe fn bytecodes(&self, method: sys::jmethodID) -> Result<JvmtiArray<'_, '_, u8>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jmethodID,
			*mut sys::jint,
			*mut *mut u8,
		) -> sys::jvmtiError = self
			.table()
			.GetBytecodes
			.ok_or(Error::MissingFunction("GetBytecodes"))?;
		let deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut count: sys::jint = 0;
		let mut pointer: *mut u8 = ptr::null_mut();
		// SAFETY: The caller supplies the method and context; outputs are writable.
		check(unsafe { function(self.as_raw(), method, &raw mut count, &raw mut pointer) })?;
		// SAFETY: Only successful calls return defined, owned JVMTI output.
		unsafe { JvmtiArray::from_output(deallocator, pointer, count, None) }
	}
}
