use super::{
	ATTACHED_THREADS, AttachGuard, Env, Error, JNIVersion, JavaVM, JniError, Ordering, Result, ScopeToken,
	THREAD_ATTACHMENT, THREAD_GUARD_NEST_LEVEL, jni_error_code_to_result, ptr, sys,
};

impl JavaVM {
	/// Returns the current number of threads attached to the JVM.
	///
	/// This only counts the thread attachments made via this library's API.
	///
	/// This method is provided mostly for diagnostic purposes / unit tests.
	///
	/// Check the docs for `ATTACHED_THREADS` for more details - including caveats for Windows with FLS.
	#[doc(hidden)]
	pub fn threads_attached(&self) -> usize {
		ATTACHED_THREADS.load(Ordering::SeqCst)
	}

	/// Returns the current nesting level for [`AttachGuard`]s
	///
	/// This is only really public since it's useful for unit tests
	#[doc(hidden)]
	#[must_use]
	pub fn thread_attach_guard_level() -> usize {
		THREAD_GUARD_NEST_LEVEL.get()
	}

	/// Get an [`AttachGuard`] for the [`Env`] associated with the current thread or, if JNI is
	/// not attached to the Java VM, this will return [`Error::JniCall`] with
	/// [`JniError::ThreadDetached`].
	///
	/// Note: jni-rs is implemented based on an assumption that all real-world implementations of
	/// JNI `GetEnv` will return the same pointer for any given version (so long as the version is
	/// supported).
	///
	/// Hypothetically the JNI spec allows for the possibility for an implementation to return a
	/// different Env pointer that nulls out functions that aren't valid for that version (or
	/// dispatches calls differently).
	///
	/// If we ever find a JVM implementation that in fact returns a different pointer then we could
	/// just repeat the `GetEnv` call with the maximum supported version after querying the version.
	///
	/// # Safety
	///
	/// You must know that the [`JavaVM`] supports at least JNI >= 1.2 (we require >= 1.4 but
	/// we couldn't even call `GetEnv` without 1.2)
	///
	/// See the 'Safety' rules for [`AttachGuard`]
	pub(crate) unsafe fn sys_get_env_attachment<'local>(&self) -> Result<AttachGuard<'local>> {
		// SAFETY: The caller upholds the guard scope rules; GetEnv initializes a thread-local pointer on success.
		unsafe {
			let mut ptr = ptr::null_mut();
			let res = java_vm_call_unchecked!(self, v1_2, GetEnv, &raw mut ptr, JNIVersion::V1_4.into());
			jni_error_code_to_result(res)?;
			let jni = ptr.cast::<sys::JNIEnv>();
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
		let env_ptr = THREAD_ATTACHMENT.get();
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
			let mut scope = ScopeToken::default();
			let mut guard = self.get_env_attachment(&mut scope)?;
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
			let mut scope = ScopeToken::default();
			let mut guard = self.get_env_attachment(&mut scope)?;
			f(guard.borrow_env_mut())
		}
	}

	/// Unloads the `JavaVM` and frees all it's associated resources
	///
	/// Firstly if this thread is not already attached to the `JavaVM` then it
	/// will be attached.
	///
	/// This thread will then wait until there are no other non-daemon threads
	/// attached to the `JavaVM` before unloading it (including threads spawned
	/// by Java and those that are attached via JNI)
	///
	/// # Safety
	///
	/// IF YOU ARE USING DAEMON THREADS THIS MAY BE DIFFICULT TO USE SAFELY!
	///
	/// ## Daemon thread rules
	///
	/// Since the JNI spec makes it clear that [`DestroyJavaVM`][destroy] will
	/// not wait for attached daemon threads to exit, this also means that if
	/// you do have any attached daemon threads it is your responsibility to
	/// ensure that they don't try and use JNI after the `JavaVM` is destroyed
	/// and you won't be able to detach them after the `JavaVM` has been
	/// destroyed.
	///
	/// This creates a very unsafe hazard if using `jni-rs` due to the various
	/// RAII types that will automatically make JNI calls within their `Drop`
	/// implementation.
	///
	/// For this reason `jni-rs` doesn't directly support attaching or detaching
	/// 'daemon' threads and it's assumed you will manage their safety yourself
	/// if you're using them.
	///
	/// Note: [`JavaVM::detach_current_thread()`] is a no-op for daemon threads
	/// because it will only detach threads that were attached via `jni-rs` APIs.
	///
	/// ## Don't call from a Java native function
	///
	/// There must be no Java methods on the call stack when `JavaVM::destroy()`
	/// is called.
	///
	/// ## Drop all JNI state, including auto-release types before calling `JavaVM::destroy()`
	///
	/// There is currently no `'vm` lifetime associated with a `JavaVM` that
	/// would allow the borrow checker to enforce that all `jni` resources
	/// associated with the `JavaVM` have been released.
	///
	/// Since these JNI resources could lead to undefined behaviour through any
	/// use after the `JavaVM` has been destroyed then it is your responsibility
	/// to release these resources.
	///
	/// In particular, there are numerous auto-release types in the `jni` API
	/// that will automatically make JNI calls within their `Drop`
	/// implementation. All such types _must_ be dropped before `destroy()` is
	/// called to avoid undefined behaviour.
	///
	/// Here is an non-exhaustive list of auto-release types to consider:
	/// - `AttachGuard`
	/// - `AutoElements`
	/// - `AutoElementsCritical`
	/// - `Auto`
	/// - `Global`
	/// - `Weak`
	/// - `MUTF8Chars`
	/// - `JMap`
	///
	/// ## Invalid `JavaVM` on return
	///
	/// After `destroy()` returns then the `JavaVM` will be in an undefined
	/// state and must be dropped (e.g. via `std::mem::drop()`) to avoid
	/// undefined behaviour.
	///
	/// This method doesn't take ownership of the `JavaVM` before it is
	/// destroyed because the `JavaVM` may have been shared (E.g. via an `Arc`)
	/// between all the threads that have not yet necessarily exited before this
	/// is called.
	///
	/// So although the `JavaVM` won't necessarily be solely owned by this
	/// thread when `destroy()` is first called it will conceptually own the
	/// `JavaVM` before `destroy()` returns.
	///
	/// [destroy]:
	///     https://docs.oracle.com/en/java/javase/12/docs/specs/jni/invocation.html#unloading-the-vm
	///
	/// # Errors
	///
	/// Returns an error if the JVM rejects or fails the shutdown request.
	pub unsafe fn destroy(&self) -> Result<()> {
		// SAFETY: The caller guarantees VM shutdown is safe and that no JNI references will be used afterward.
		unsafe {
			let res = java_vm_call_unchecked!(self, v1_1, DestroyJavaVM);
			jni_error_code_to_result(res)
		}
	}
}
