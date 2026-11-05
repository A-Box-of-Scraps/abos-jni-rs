use super::*;

impl<'local> Env<'local> {
	/// Returns an `UnsupportedVersion` error if the current JNI version is
	/// lower than the one given.
	///
	/// Returns `JavaException` if called while there is a pending exception.
	#[allow(unused)]
	pub(super) fn ensure_version(&self, version: JNIVersion) -> Result<()> {
		if self.version()? < version {
			Err(Error::UnsupportedVersion)
		} else {
			Ok(())
		}
	}

	/// Create a [`Env`] associated with single JNI local reference frame.
	///
	/// `level` represents the nesting level of the [`AttachGuard`] that owns
	/// this [`Env`]. This can be compared to [JavaVM::thread_attach_guard_level()]
	/// to check that this [`Env`] is associated with the top-most JNI stack frame.
	///
	/// `owns_attachment` should be true if this [`Env`] is associated with an
	/// attachment that will detach the thread when dropped.
	pub(crate) unsafe fn new(raw: *mut sys::JNIEnv, level: usize, owns_attachment: bool) -> Self {
		let env = Env {
			raw,
			level,
			owns_attachment,
			_lifetime: PhantomData,
		};
		// Assuming that the application doesn't break the safety rules for
		// keeping the `AttachGuard` on the stack, and not re-ordering them,
		// we can assert that we only ever create an `Env` for the top-most
		// guard on the stack.
		env.assert_top();
		env
	}

	/// Runtime check that this [`Env`] represents the top JNI stack frame
	///
	/// Any lower-level API that returns a new local reference must call this
	/// method to ensure the reference is tied to the correct JNI stack frame.
	///
	/// All safe APIs that return a new local reference already call this but you
	/// may need to call this in `unsafe` code that uses [`crate::sys`] functions
	/// directly.
	///
	/// See the safety documentation for [`AttachGuard`] for more details.
	#[inline]
	pub fn assert_top(&self) {
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		assert_eq!(
			self.level,
			JavaVM::thread_attach_guard_level(),
			r#" jni runtime check failure: attempt to create a new local reference using an Env<'not_top> that is not associated with the top JNI stack frame.

This implies that there is more than one mutable Env in scope.

This should only be possible by misusing APIs like JavaVM::attach_current_thread or Env::with_local_frame to materialize a mutable Env when there is already one in scope.

To fix this, ensure that you only ever have one mutable Env in scope at a time, which is associated with the top JNI stack frame.

See the jni-rs Env documentation for more details.
"#
		);
	}

	/// Returns true if this [`Env`] is associated with a scoped attachment that
	/// will also detach the thread when it is dropped.
	///
	/// An owned attachment would come from a call to
	/// [`JavaVM::attach_current_thread_for_scope`] but only if the thread was
	/// not already attached.
	///
	/// This can be used to recognise when
	/// [`JavaVM::attach_current_thread_for_scope`] really needed to attach the
	/// thread, or if the thread was already attached.
	///
	/// This is mostly useful for diagnostic purposes. For example the `Drop`
	/// implementation for [`Global`] and [`Weak`] will print a warning if they
	/// are dropped on a thread that is not attached, which is recognised with
	/// this method.
	pub fn owns_attachment(&self) -> bool {
		self.owns_attachment
	}

	/// Get the raw Env pointer
	pub fn get_raw(&self) -> *mut sys::JNIEnv {
		self.raw
	}

	/// Get the JNI version that this [`Env`] supports.
	///
	/// This can return an error if called while there is a pending JNI exception
	pub fn version(&self) -> Result<JNIVersion> {
		// Safety: GetVersion is 1.1 API that must be valid
		Ok(JNIVersion::from(unsafe {
			jni_call_no_post_check_ex!(self, v1_1, GetVersion)?
		}))
	}

	#[doc(hidden)]
	#[deprecated(since = "0.22.0", note = "Renamed to `version` instead")]
	pub fn get_version(&self) -> Result<JNIVersion> {
		self.version()
	}
}
