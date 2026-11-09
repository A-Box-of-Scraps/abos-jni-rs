use core::{ffi::c_char, ptr};

use abos_jni::strings::JNIStr;

use crate::{
	Env, Error, JvmtiArray, JvmtiString, JvmtiStringList, LocalVariableTable, Result, api::array::Deallocator,
	internal::error::check, sys,
};

impl Env<'_> {
	/// Returns the VM's system-property keys, owning each string and the array.
	///
	/// Available in JVMTI 1.2; no capability is required. These are VM properties,
	/// not necessarily all properties visible through `java.lang.System`.
	///
	/// # Errors
	/// Returns a native error, missing function, or invalid output error.
	///
	/// # Safety
	/// Call in `OnLoad` or the live phase on a JVMTI-permitted thread, outside
	/// restricted callbacks (not callback-safe). Keep the environment and VM alive
	/// through cleanup and satisfy [`JvmtiArray`]'s cleanup-context requirements.
	pub unsafe fn system_properties(&self) -> Result<JvmtiStringList<'_, '_>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			*mut sys::jint,
			*mut *mut *mut c_char,
		) -> sys::jvmtiError = self
			.table()
			.GetSystemProperties
			.ok_or(Error::MissingFunction("GetSystemProperties"))?;
		let deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut count: sys::jint = 0;
		let mut pointer: *mut *mut c_char = ptr::null_mut();
		// SAFETY: The caller supplies the execution context and outputs are writable.
		check(unsafe { function(self.as_raw(), &raw mut count, &raw mut pointer) })?;
		// On native error outputs are undefined and must not be inspected or freed.
		// SAFETY: A successful call returns separately owned strings and their array.
		unsafe { JvmtiStringList::from_output(deallocator, pointer, count) }
	}

	/// Returns a VM system-property value as an owned modified UTF-8 string.
	///
	/// Available in JVMTI 1.2; no capability is required.
	///
	/// # Errors
	/// Returns a native error (including `NOT_AVAILABLE` for an unknown key), a
	/// missing function, or an invalid output error.
	///
	/// # Safety
	/// The phase, callback, thread, and cleanup rules of [`Self::system_properties`] apply.
	pub unsafe fn system_property(&self, key: &JNIStr) -> Result<JvmtiString<'_, '_>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			*const c_char,
			*mut *mut c_char,
		) -> sys::jvmtiError = self
			.table()
			.GetSystemProperty
			.ok_or(Error::MissingFunction("GetSystemProperty"))?;
		let _deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut pointer: *mut c_char = ptr::null_mut();
		// SAFETY: The key is terminated, output is writable, and caller supplies the context.
		check(unsafe { function(self.as_raw(), key.as_ptr(), &raw mut pointer) })?;
		// SAFETY: Success supplies ownership of a terminated modified UTF-8 string.
		unsafe { JvmtiString::from_raw(self, pointer) }
	}

	/// Returns an owned line-number table for a method (JVMTI 1.2).
	///
	/// # Errors
	/// Returns native errors, including missing `can_get_line_numbers` capability,
	/// absent debug information, or a native method, and wrapper output errors.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted thread, outside restricted
	/// callbacks (not callback-safe). `method` must remain valid during the call.
	/// [`JvmtiArray`]'s environment and cleanup-context requirements apply.
	pub unsafe fn line_number_table(
		&self,
		method: sys::jmethodID,
	) -> Result<JvmtiArray<'_, '_, sys::jvmtiLineNumberEntry>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jmethodID,
			*mut sys::jint,
			*mut *mut sys::jvmtiLineNumberEntry,
		) -> sys::jvmtiError = self
			.table()
			.GetLineNumberTable
			.ok_or(Error::MissingFunction("GetLineNumberTable"))?;
		let deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut count: sys::jint = 0;
		let mut pointer: *mut sys::jvmtiLineNumberEntry = ptr::null_mut();
		// SAFETY: Method and context are supplied by caller; outputs are writable.
		check(unsafe { function(self.as_raw(), method, &raw mut count, &raw mut pointer) })?;
		// SAFETY: Success supplies an initialized flat allocation with no nested ownership.
		unsafe { JvmtiArray::from_output(deallocator, pointer, count, None) }
	}

	/// Returns a local-variable table, owning all nested strings (JVMTI 1.2).
	///
	/// # Errors
	/// Returns native errors, including missing `can_access_local_variables`
	/// capability, absent debug information, or a native method, and wrapper errors.
	///
	/// # Safety
	/// The phase, callback, thread, method-validity, and cleanup rules of
	/// [`Self::line_number_table`] apply.
	pub unsafe fn local_variable_table(&self, method: sys::jmethodID) -> Result<LocalVariableTable<'_, '_>> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jmethodID,
			*mut sys::jint,
			*mut *mut sys::jvmtiLocalVariableEntry,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalVariableTable
			.ok_or(Error::MissingFunction("GetLocalVariableTable"))?;
		let deallocator: Deallocator<'_, '_> = Deallocator::new(self)?;
		let mut count: sys::jint = 0;
		let mut pointer: *mut sys::jvmtiLocalVariableEntry = ptr::null_mut();
		// SAFETY: Method and context are supplied by caller; outputs are writable.
		check(unsafe { function(self.as_raw(), method, &raw mut count, &raw mut pointer) })?;
		// SAFETY: Success supplies entries with individually allocated string fields.
		unsafe { LocalVariableTable::from_output(deallocator, pointer, count) }
	}
}
