use crate::{Env, Error, Result, internal::error::check, sys};

impl Env<'_> {
	/// Reads a object local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// The returned object is a nullable JNI local reference; manage it separately
	/// in this thread's local frame, not with JVMTI deallocation.
	pub unsafe fn get_local_object(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
	) -> Result<sys::jobject> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jobject,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalObject
			.ok_or(Error::MissingFunction("GetLocalObject"))?;
		let mut value: sys::jobject = core::ptr::null_mut();
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, slot, &raw mut value) })?;
		Ok(value)
	}

	/// Writes a object local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// The value must be null or a valid JNI reference assignable to the local.
	/// It is borrowed during the call; no reference ownership is transferred.
	pub unsafe fn set_local_object(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
		value: sys::jobject,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			sys::jobject,
		) -> sys::jvmtiError = self
			.table()
			.SetLocalObject
			.ok_or(Error::MissingFunction("SetLocalObject"))?;
		// SAFETY: Caller supplies the valid frame, value, and execution context.
		check(unsafe { function(self.as_raw(), thread, depth, slot, value) })
	}

	/// Reads a int local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn get_local_int(&self, thread: sys::jthread, depth: sys::jint, slot: sys::jint) -> Result<sys::jint> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jint,
		) -> sys::jvmtiError = self.table().GetLocalInt.ok_or(Error::MissingFunction("GetLocalInt"))?;
		let mut value: sys::jint = 0;
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, slot, &raw mut value) })?;
		Ok(value)
	}

	/// Writes a int local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn set_local_int(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
		value: sys::jint,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			sys::jint,
		) -> sys::jvmtiError = self.table().SetLocalInt.ok_or(Error::MissingFunction("SetLocalInt"))?;
		// SAFETY: Caller supplies the valid frame, value, and execution context.
		check(unsafe { function(self.as_raw(), thread, depth, slot, value) })
	}

	/// Reads a long local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn get_local_long(&self, thread: sys::jthread, depth: sys::jint, slot: sys::jint) -> Result<sys::jlong> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jlong,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalLong
			.ok_or(Error::MissingFunction("GetLocalLong"))?;
		let mut value: sys::jlong = 0;
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, slot, &raw mut value) })?;
		Ok(value)
	}

	/// Writes a long local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn set_local_long(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
		value: sys::jlong,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			sys::jlong,
		) -> sys::jvmtiError = self
			.table()
			.SetLocalLong
			.ok_or(Error::MissingFunction("SetLocalLong"))?;
		// SAFETY: Caller supplies the valid frame, value, and execution context.
		check(unsafe { function(self.as_raw(), thread, depth, slot, value) })
	}

	/// Reads a float local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn get_local_float(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
	) -> Result<sys::jfloat> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jfloat,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalFloat
			.ok_or(Error::MissingFunction("GetLocalFloat"))?;
		let mut value: sys::jfloat = 0.0;
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, slot, &raw mut value) })?;
		Ok(value)
	}

	/// Writes a float local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn set_local_float(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
		value: sys::jfloat,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			sys::jfloat,
		) -> sys::jvmtiError = self
			.table()
			.SetLocalFloat
			.ok_or(Error::MissingFunction("SetLocalFloat"))?;
		// SAFETY: Caller supplies the valid frame, value, and execution context.
		check(unsafe { function(self.as_raw(), thread, depth, slot, value) })
	}

	/// Reads a double local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn get_local_double(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
	) -> Result<sys::jdouble> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			*mut sys::jdouble,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalDouble
			.ok_or(Error::MissingFunction("GetLocalDouble"))?;
		let mut value: sys::jdouble = 0.0;
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, slot, &raw mut value) })?;
		Ok(value)
	}

	/// Writes a double local variable (JVMTI 1.2; `can_access_local_variables`).
	///
	/// # Errors
	/// Returns native errors including invalid slot/depth, type mismatch, opaque
	/// frame, missing capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside
	/// restricted callbacks (not callback-safe). The thread reference must be valid;
	/// null selects the current thread. The target must be current or suspended.
	/// Coordinate suspension/resumption and frame changes with other agent users.
	/// Only a primitive value is copied; there is no allocation ownership.
	pub unsafe fn set_local_double(
		&self,
		thread: sys::jthread,
		depth: sys::jint,
		slot: sys::jint,
		value: sys::jdouble,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			sys::jint,
			sys::jdouble,
		) -> sys::jvmtiError = self
			.table()
			.SetLocalDouble
			.ok_or(Error::MissingFunction("SetLocalDouble"))?;
		// SAFETY: Caller supplies the valid frame, value, and execution context.
		check(unsafe { function(self.as_raw(), thread, depth, slot, value) })
	}

	/// Returns the receiver of an instance frame (JVMTI 1.2).
	///
	/// Requires `can_access_local_variables`. Unlike slot access this also supports
	/// native instance frames. The result is an independently managed JNI local reference.
	///
	/// # Errors
	/// Returns native errors including invalid depth, a static frame, missing
	/// capability, or unsuspended thread, or a missing function.
	///
	/// # Safety
	/// The phase, thread, callback, suspension, and JNI ownership requirements of
	/// [`Self::get_local_object`] apply.
	pub unsafe fn local_instance(&self, thread: sys::jthread, depth: sys::jint) -> Result<sys::jobject> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jthread,
			sys::jint,
			*mut sys::jobject,
		) -> sys::jvmtiError = self
			.table()
			.GetLocalInstance
			.ok_or(Error::MissingFunction("GetLocalInstance"))?;
		let mut value: sys::jobject = core::ptr::null_mut();
		// SAFETY: Caller supplies the valid frame and context; output is writable.
		check(unsafe { function(self.as_raw(), thread, depth, &raw mut value) })?;
		Ok(value)
	}
}
