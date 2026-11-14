use super::{
	ATTACHED_THREADS, AttachGuard, Env, Error, JNIVersion, JObject, JavaVM, JniError, Ordering, Result, ScopeToken,
	THREAD_ATTACHMENT, THREAD_GUARD_NEST_LEVEL, jni_error_code_to_result, ptr, sys,
};

impl JavaVM {
	/// Diagnostic count of this crate's attachment records; see `ATTACHED_THREADS`
	/// for Windows fiber-counting semantics.
	#[doc(hidden)]
	pub fn threads_attached(&self) -> usize {
		ATTACHED_THREADS.load(Ordering::SeqCst)
	}

	/// Returns the current nesting level for [`AttachGuard`]s.
	#[doc(hidden)]
	#[must_use]
	pub fn thread_attach_guard_level() -> usize {
		THREAD_GUARD_NEST_LEVEL.get()
	}

	/// Deletes an owned local reference using the active guard's JNI environment when available.
	#[inline]
	pub(crate) fn delete_local_ref(obj: JObject<'_>) {
		// Null references can exist before any VM or thread attachment does.
		if obj.is_null() {
			return;
		}
		let env: *mut sys::JNIEnv = THREAD_ATTACHMENT.get();
		if env.is_null() {
			Self::delete_local_ref_without_guard(obj);
			return;
		}
		// SAFETY: THREAD_ATTACHMENT is populated only while an AttachGuard keeps this
		// thread attached, and is cleared when the last guard exits. The consumed
		// object owns a local reference on this thread. DeleteLocalRef is exception
		// safe and can delete references from outer frames; it creates no references
		// and needs neither a new guard nor an assert_top check.
		unsafe { ((**env).v1_1.DeleteLocalRef)(env, obj.into_raw()) };
	}

	#[cold]
	fn delete_local_ref_without_guard(obj: JObject<'_>) {
		let Ok(vm) = Self::singleton() else {
			// A native method argument may be wrapped before the first library guard
			// initializes the singleton. Its native frame will still release it.
			log::error!("Failed to drop Auto: No JavaVM initialized");
			return;
		};
		// Native arguments can outlive a library guard without outliving their JNI
		// frame. Query the current attachment rather than caching an inactive one.
		vm.with_top_local_frame(|env| -> Result<()> {
			env.delete_local_ref(obj);
			Ok(())
		})
		.expect("Auto local reference outlived its thread attachment");
	}

	/// Borrow the current thread's attachment, or return `JniError::ThreadDetached`.
	///
	/// Assumes `GetEnv` returns the same environment pointer for all supported JNI versions.
	///
	/// # Safety
	///
	/// The VM must support JNI 1.4. The caller must uphold [`AttachGuard`]'s scope rules.
	pub(crate) unsafe fn sys_get_env_attachment<'local>(&self) -> Result<AttachGuard<'local>> {
		// SAFETY: The caller upholds the guard scope rules; GetEnv initializes a thread-local pointer on success.
		unsafe {
			let mut ptr: *mut std::ffi::c_void = ptr::null_mut();
			let res: i32 = java_vm_call_unchecked!(self, v1_2, GetEnv, &raw mut ptr, JNIVersion::V1_4.into());
			jni_error_code_to_result(res)?;
			let jni: *mut *const abos_jni_sys::JNINativeInterface_ = ptr.cast::<sys::JNIEnv>();
			Ok(AttachGuard::from_unowned(jni))
		}
	}

	/// Returns `true` if the current thread is attached to a Java VM.
	///
	/// Since this calls [`sys::JNIInvokeInterface__1_2::GetEnv`], it will also recognize thread
	/// attachments that made without using this crate (such as other JNI language bindings).
	///
	/// # Errors
	///
	/// Returns an error for unexpected JNI failures while querying the current thread's environment.
	pub fn is_thread_attached(&self) -> Result<bool> {
		// Safety: we aren't materializing an attachment guard while we already have access to one
		unsafe {
			self.sys_get_env_attachment()
				.map(|_| true)
				.or_else(|jni_err| match jni_err {
					Error::JniCall(JniError::ThreadDetached) => Ok(false),
					_ => Err(jni_err),
				})
		}
	}

	/// Returns an [`AttachGuard`] for the [`Env`] associated with the current thread, if there is
	/// already at least one `AttachGuard` on the stack for the current thread.
	///
	/// Returns None if there is no `AttachGuard` on the stack for the current thread.
	///
	/// This serves a similar purpose to [`sys::JNIInvokeInterface__1_2::GetEnv`] in that it
	/// provides access to the current thread's JNI environment if JNI is attached.
	///
	/// This API will only recognize attachments made by this crate (including uses of
	/// [`AttachGuard::from_unowned`]). I.e. the implementation only checks crate-specific thread
	/// local storage and will not actually call [`sys::JNIInvokeInterface__1_2::GetEnv`].
	///
	/// More-specifically, this will only recognise attachments while there is an `AttachGuard` on
	/// the stack for the current thread. Once all `AttachGuard`s for the current thread have been
	/// dropped, this API will return `None` even if a permanent attachment exists, that is being
	/// tracked via TLS/FLS outside of an `AttachGuard`.
	///
	/// This API can be used speculatively before calling `sys_get_env_attachment` to avoid making a
	/// JNI call to query the current JNI environment.
	///
	/// # Safety
	///
	/// See the 'Safety' rules for [`AttachGuard`]
	pub(crate) unsafe fn try_get_nested_env_attach_guard<'local>() -> Option<AttachGuard<'local>> {
		let env_ptr: *mut *const abos_jni_sys::JNINativeInterface_ = THREAD_ATTACHMENT.get();
		if env_ptr.is_null() {
			None
		} else {
			// Safety: we can assume any THREAD_ATTACHMENT pointer is valid
			unsafe { Some(AttachGuard::from_unowned(env_ptr)) }
		}
	}

	/// Returns an [`AttachGuard`] for top JNI stack frame attached to the current thread.
	///
	/// If the current thread is not attached to the Java VM, this will return [`Error::JniCall`]
	/// with [`JniError::ThreadDetached`].
	///
	/// This serves a similar purpose to [`sys::JNIInvokeInterface__1_2::GetEnv`] in that it
	/// provides access to the current thread's JNI environment if JNI is attached.
	///
	/// This API can recognize attachments made by other JNI language bindings but will first check
	/// crate-specific thread local storage for an attachment before calling
	/// [`sys::JNIInvokeInterface__1_2::GetEnv`].
	///
	/// Consider using [`Self::with_local_frame`] or [`Self::with_top_local_frame`] as safe
	/// alternatives for running code against the currently attached JNI environment.
	///
	/// # Safety
	///
	/// See the 'Safety' rules for [`AttachGuard`]
	///
	/// # Errors
	///
	/// Returns an error if the current thread is detached or JNI cannot obtain its environment.
	pub unsafe fn get_env_attachment<'scope>(&self, _scope: &'scope mut ScopeToken) -> Result<AttachGuard<'scope>> {
		// SAFETY: The caller upholds the AttachGuard scope rules for the returned guard.
		unsafe { Self::try_get_nested_env_attach_guard().map_or_else(|| self.sys_get_env_attachment(), Ok) }
	}

	#[doc(hidden)]
	#[deprecated(
		since = "0.22.0",
		note = "use JavaVM::with_top_local_frame, JavaVM::attach_current_thread or JavaVM::get_env_attachment instead"
	)]
	pub unsafe fn get_env<'scope>(&self, scope: &'scope mut ScopeToken) -> Result<AttachGuard<'scope>> {
		// SAFETY: The caller upholds the same scope requirements as get_env_attachment.
		unsafe { self.get_env_attachment(scope) }
	}

	/// Runs a closure within a new JNI stack frame that will be unwound to release all local
	/// references created within the given closure.
	///
	/// If the current thread is not attached to the Java VM, this will return [`Error::JniCall`]
	/// with [`JniError::ThreadDetached`].
	///
	/// This API can recognize attachments made by other JNI language bindings but will first check
	/// crate-specific thread local storage for an attachment before calling
	/// [`sys::JNIInvokeInterface__1_2::GetEnv`].
	///
	/// Internally this calls [`sys::JNINativeInterface__1_2::PushLocalFrame`] with the given
	/// `capacity`, to create a new JNI stack frame, and calls
	/// [`sys::JNINativeInterface__1_2::PopLocalFrame`] after the closure is executed.
	///
	/// If you're not sure that the current thread is attached to the Java VM, consider using
	/// [`Self::attach_current_thread`] instead.
	///
	/// # Runtime Top Frame Checks
	///
	/// See [Env] documentation for rules on limiting yourself to one [Env] reference per-scope to
	/// avoid exposing code to runtime checks for the top JNI frame (that can panic).
	///
	/// # Errors
	///
	/// Returns an error if the current thread is detached, its environment or requested local frame is unavailable, or the callback fails.
	pub fn with_local_frame<F, T, E>(&self, capacity: usize, f: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		// SAFETY: The scope token and guard stay on this stack, and the callback cannot outlive the local frame.
		unsafe {
			let mut scope: ScopeToken = ScopeToken::default();
			let mut guard: AttachGuard<'_> = self.get_env_attachment(&mut scope)?;
			guard.borrow_env_mut().with_local_frame(capacity, f)
		}
	}

	/// Runs a closure with an [`Env`] reference associated with the top JNI stack frame.
	///
	/// Unlike [`Self::with_local_frame()`], this API does not push a new JNI stack frame.
	///
	/// Most of the time this API should probably be avoided (see [`Self::with_local_frame`] or
	/// [`Self::attach_current_thread`] instead) unless you're sure your code won't leak local
	/// references into the current stack frame (or you're sure that the leaked references are
	/// acceptable because you know when the top frame will unwind and release those references).
	///
	/// This will have a slightly lower overhead than [`Self::with_local_frame()`] (since it doesn't
	/// need to push/pop a JNI stack frame), but the trade off is that you may leak local references
	/// into the top stack frame.
	///
	/// Keep in mind that deleting local references individually is likely to have a higher cost
	/// than pushing/popping a JNI stack frame, so you should probably only use this API if you're
	/// OK with leaking a small number of local references into the top frame.
	///
	/// # Runtime Top Frame Checks
	///
	/// See [Env] documentation for rules on limiting yourself to one [Env] reference per-scope to
	/// avoid exposing code to runtime checks for the top JNI frame (that can panic).
	///
	/// # Errors
	///
	/// Returns an error if the current thread is detached, its environment or requested local frame is unavailable, or the callback fails.
	pub fn with_top_local_frame<F, T, E>(&self, f: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		// SAFETY: The scope token and guard stay on this stack and outlive the callback.
		unsafe {
			let mut scope: ScopeToken = ScopeToken::default();
			let mut guard: AttachGuard<'_> = self.get_env_attachment(&mut scope)?;
			f(guard.borrow_env_mut())
		}
	}

	/// Destroys the VM, attaching the calling thread if necessary and waiting for
	/// other non-daemon threads, including Java threads, to exit.
	///
	/// # Safety
	///
	/// - No Java methods may be on the calling thread's stack. Do not call from a
	///   Java native method.
	/// - Release all JNI resources before calling. Rust lifetimes do not enforce
	///   the VM's lifetime. In particular, drop guards, `Auto`, `Global`, `Weak`,
	///   array-element guards, and `MUTF8Chars`, whose destructors call JNI.
	/// - Coordinate with daemon threads: destruction does not wait for them, and
	///   they must neither use JNI nor attempt detachment after destruction. This
	///   crate does not manage daemon attachments created outside its APIs.
	/// - After success, do not use this handle, any copy, or the cached singleton
	///   for JNI calls. The singleton is not reset; creating another VM is unsupported.
	///
	/// Dropping a `JavaVM` handle does not call JNI or destroy the VM.
	///
	/// # Errors
	///
	/// Returns an error if the JVM rejects or fails the shutdown request.
	pub unsafe fn destroy(&self) -> Result<()> {
		// SAFETY: The caller guarantees VM shutdown is safe and that no JNI references will be used afterward.
		unsafe {
			let res: i32 = java_vm_call_unchecked!(self, v1_1, DestroyJavaVM);
			jni_error_code_to_result(res)
		}
	}
}
