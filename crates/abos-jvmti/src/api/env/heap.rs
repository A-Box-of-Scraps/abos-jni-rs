use core::ffi::c_void;

use crate::{Env, Error, Result, internal::error::check, sys};

impl Env<'_> {
	/// Returns this environment's object tag (JVMTI 1.2; `can_tag_objects`).
	///
	/// Zero means untagged. Tags neither own objects nor prevent collection.
	///
	/// # Errors
	/// Returns native capability/object errors or a missing function.
	///
	/// # Safety
	/// Call in the start or live phase on a JVMTI-permitted attached thread,
	/// outside restricted callbacks (not callback-safe). The object must be a valid
	/// non-null JNI reference. Coordinate tag changes with other environment users.
	pub unsafe fn tag(&self, object: sys::jobject) -> Result<sys::jlong> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jobject, *mut sys::jlong) -> sys::jvmtiError =
			self.table().GetTag.ok_or(Error::MissingFunction("GetTag"))?;
		let mut tag: sys::jlong = 0;
		// SAFETY: Caller supplies the object and context; output is writable.
		check(unsafe { function(self.as_raw(), object, &raw mut tag) })?;
		Ok(tag)
	}

	/// Sets this environment's object tag (JVMTI 1.2; `can_tag_objects`).
	///
	/// Zero removes a tag; neither JNI nor Rust ownership is transferred.
	///
	/// # Errors
	/// Returns native capability/object errors or a missing function.
	///
	/// # Safety
	/// The phase, callback, thread, reference, and coordination rules of [`Self::tag`] apply.
	pub unsafe fn set_tag(&self, object: sys::jobject, tag: sys::jlong) -> Result<()> {
		let function: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jobject, sys::jlong) -> sys::jvmtiError =
			self.table().SetTag.ok_or(Error::MissingFunction("SetTag"))?;
		// SAFETY: Caller supplies the valid object and context.
		check(unsafe { function(self.as_raw(), object, tag) })
	}

	/// Visits heap objects synchronously (JVMTI 1.2; `can_tag_objects`).
	///
	/// `filter` combines JVMTI heap-filter exclusion flags. A null class visits all
	/// classes. The callback table and user data are borrowed only until return.
	/// Callback arguments are borrowed VM data, never JVMTI-owned allocations.
	///
	/// # Errors
	/// Returns native filter/class/capability errors or a missing function.
	///
	/// # Safety
	/// Call in the live phase on a JVMTI-permitted attached thread, outside restricted
	/// callbacks (not callback-safe). A non-null class must be a valid JNI reference.
	/// Reserved callback slots must be null. Callbacks must not unwind or call JNI
	/// or non-callback-safe JVMTI functions. User data must remain valid for callback
	/// access, with aliasing and synchronization handled by the caller. Do not
	/// retain callback-scoped pointers; tag pointers may only be mutated as specified
	/// by JVMTI. Heap traversal stalls Java execution, so avoid blocking on Java work.
	pub unsafe fn iterate_through_heap(
		&self,
		filter: sys::jint,
		class: sys::jclass,
		callbacks: &sys::jvmtiHeapCallbacks,
		user_data: *mut c_void,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jint,
			sys::jclass,
			*const sys::jvmtiHeapCallbacks,
			*const c_void,
		) -> sys::jvmtiError = self
			.table()
			.IterateThroughHeap
			.ok_or(Error::MissingFunction("IterateThroughHeap"))?;
		// SAFETY: Caller supplies valid synchronous callbacks, references, data, and execution context.
		check(unsafe { function(self.as_raw(), filter, class, callbacks, user_data) })
	}

	/// Follows object references synchronously (JVMTI 1.2; `can_tag_objects`).
	///
	/// Null `initial_object` starts from heap roots. `filter` excludes reported
	/// objects, not traversal through them; callback return flags control traversal.
	/// Class, callback table, and user data are borrowed, not retained or freed.
	///
	/// # Errors
	/// Returns native object/class/filter/capability errors or a missing function.
	///
	/// # Safety
	/// The phase, callback, thread, aliasing, and ownership requirements of
	/// [`Self::iterate_through_heap`] apply. A non-null initial object must also be
	/// a valid JNI reference. Inspect reference-info unions only by their supplied kind.
	pub unsafe fn follow_references(
		&self,
		filter: sys::jint,
		class: sys::jclass,
		initial_object: sys::jobject,
		callbacks: &sys::jvmtiHeapCallbacks,
		user_data: *mut c_void,
	) -> Result<()> {
		let function: unsafe extern "system" fn(
			*mut sys::jvmtiEnv,
			sys::jint,
			sys::jclass,
			sys::jobject,
			*const sys::jvmtiHeapCallbacks,
			*const c_void,
		) -> sys::jvmtiError = self
			.table()
			.FollowReferences
			.ok_or(Error::MissingFunction("FollowReferences"))?;
		// SAFETY: Caller supplies valid synchronous callbacks, references, data, and execution context.
		check(unsafe { function(self.as_raw(), filter, class, initial_object, callbacks, user_data) })
	}
}
