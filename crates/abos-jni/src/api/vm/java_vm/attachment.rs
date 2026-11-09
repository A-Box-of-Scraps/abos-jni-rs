use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use super::{
	AttachConfig, AttachGuard, AttachmentExceptionPolicy, DEFAULT_LOCAL_FRAME_CAPACITY, Env, Error, JavaVM, JniError,
	Result, ScopeToken, THREAD_GUARD_NEST_LEVEL, current, sys_attach_current_thread,
};
#[cfg(use_fls_attach_guard)]
use crate::internal::fls_attach_guard;
#[cfg(use_tls_attach_guard)]
use crate::internal::tls_attach_guard;

impl JavaVM {
	/// Attaches the current thread to the Java VM and calls the provided callback with a mutable
	/// [`Env`] reference.
	///
	/// If the thread was not already attached then a new attachment is made which will be
	/// automatically detached when the current thread terminates.
	///
	/// Calling this in a thread that is already attached is cheap since it will only need to check
	/// thread local storage without making a JNI call.
	///
	/// This API requests to permanently attach the current thread but since pre-existing
	/// attachments aren't affected by this API, it should not be assumed that the thread will
	/// definitely remain attached until it exits - that is only a request. For example if something
	/// higher on the stack has created a scoped attachment then that will take precedence and the
	/// thread will not be permanently attached.
	///
	/// You can safely assume that the thread will remain attached for the duration of the callback.
	///
	/// If you're not sure whether to use [`Self::attach_current_thread`] or
	/// [`Self::attach_current_thread_for_scope`], then you should probably use this API because it
	/// increases the chance that future attachment calls will be cheap.
	///
	/// # Exception Handling
	///
	/// If the thread is already attached and has a pending exception (before the callback is run),
	/// the exception will be stashed and re-thrown after the callback completes to avoid side
	/// effects and to also ensure your callback can run un-affected by the pending exception. (this
	/// never affects the return value). The original exception is also restored if the callback
	/// panics and unwinds.
	///
	/// If your callback throws an exception this will be caught and returned as an
	/// [`Error::CaughtJavaException`].
	///
	/// The exception handling policy can be changed via [`AttachConfig::exceptions_policy`] and
	/// using [`JavaVM::attach_current_thread_with_config`].
	///
	/// # Runtime Top Frame Checks
	///
	/// See [Env] documentation for rules on limiting yourself to one [Env] reference per-scope to
	/// avoid exposing code to runtime checks for the top JNI frame that can panic.
	///
	/// # Errors
	///
	/// Returns an error if attachment or local-frame setup fails, the callback returns an error, or the configured exception policy catches a Java exception.
	pub fn attach_current_thread<F, T, E>(&self, callback: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		self.attach_current_thread_with_config(AttachConfig::default, Some(DEFAULT_LOCAL_FRAME_CAPACITY), callback)
	}

	/// Attaches the current thread to the Java VM and calls the provided callback with a mutable
	/// [`Env`] reference.
	///
	/// If the thread was not already attached, the thread will be detached when the callback
	/// returns.
	///
	/// Calling this in a thread that is already attached is cheap since it will only need to check
	/// thread local storage without making a JNI call.
	///
	/// Attaching a thread is an expensive operation if it was not already attached, so it's
	/// generally recommended that you should use [`Self::attach_current_thread()`] (requesting to
	/// attach the thread permanently) instead of using a scoped attachment. Using this API may
	/// increase the chance that you incur the cost of repeatedly attaching and detaching the same
	/// thread.
	///
	/// # Exception Handling
	///
	/// If the thread is already attached and has a pending exception (before the callback is run),
	/// the exception will be stashed and re-thrown after the callback completes to avoid side
	/// effects and to also ensure your callback can run un-affected by the pending exception. (this
	/// never affects the return value). The original exception is also restored if the callback
	/// panics and unwinds.
	///
	/// If your callback throws an exception this will be caught and returned as an
	/// [`Error::CaughtJavaException`].
	///
	/// The exception handling policy can be changed via [`AttachConfig::exceptions_policy`] and
	/// using [`JavaVM::attach_current_thread_with_config`].
	///
	/// # Runtime Top Frame Checks
	///
	/// See [Env] documentation for rules on limiting yourself to one [Env] reference per-scope to
	/// avoid exposing code to runtime checks for the top JNI frame that can panic.
	///
	/// # Errors
	///
	/// Returns an error if attachment or local-frame setup fails, the callback returns an error, or the configured exception policy catches a Java exception.
	pub fn attach_current_thread_for_scope<F, T, E>(&self, callback: F) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		self.attach_current_thread_with_config(
			|| AttachConfig::new().scoped(true),
			Some(DEFAULT_LOCAL_FRAME_CAPACITY),
			callback,
		)
	}

	/// Attaches the current thread to the Java VM and calls the provided callback with a mutable
	/// [`Env`] reference.
	///
	/// This function allows you to customize the attachment process and choose whether to create
	/// a new local frame (with a given `Some(capacity)`) or use the current one (`None`).
	///
	/// Most of the time you should prefer to use [`Self::attach_current_thread()`] or
	/// [`Self::attach_current_thread_for_scope()`] instead of this function.
	///
	/// The semantics of [`Self::attach_current_thread`] are equivalent to:
	/// ```rust,ignore-aarch64-linux-android
	/// # use abos_jni::{JavaVM, AttachConfig, DEFAULT_LOCAL_FRAME_CAPACITY};
	/// # fn jni_example(vm: &JavaVM) -> abos_jni::errors::Result<()> {
	///      vm.attach_current_thread_with_config(AttachConfig::default, Some(DEFAULT_LOCAL_FRAME_CAPACITY), |env| {
	///          // Use the Env reference
	///          Ok(())
	///      })
	/// # }
	/// ```
	///
	/// The semantics of [`Self::attach_current_thread_for_scope`] are equivalent to:
	/// ```rust,ignore-aarch64-linux-android
	/// # use abos_jni::{JavaVM, AttachConfig, DEFAULT_LOCAL_FRAME_CAPACITY};
	/// # fn jni_example(vm: &JavaVM) -> abos_jni::errors::Result<()> {
	///      vm.attach_current_thread_with_config(|| AttachConfig::default().scoped(true), Some(DEFAULT_LOCAL_FRAME_CAPACITY), |env| {
	///          // Use the Env reference
	///          Ok(())
	///      })
	/// # }
	/// ```
	///
	/// See [`Self::attach_current_thread`], [`Self::attach_current_thread_for_scope`] and [`AttachConfig`] for more details.
	///
	/// # Runtime Top Frame Checks
	///
	/// See [Env] documentation for rules on limiting yourself to one [Env] reference per-scope to
	/// avoid exposing code to runtime checks for the top JNI frame that can panic.
	///
	/// # Errors
	///
	/// Returns an error if attachment or local-frame setup fails, the callback returns an error, or the configured exception policy catches a Java exception.
	pub fn attach_current_thread_with_config<'config, F, C, T, E>(
		&self,
		config: C,
		capacity: Option<usize>,
		callback: F,
	) -> std::result::Result<T, E>
	where
		F: FnOnce(&mut Env) -> std::result::Result<T, E>,
		E: From<Error>,
		C: FnOnce() -> AttachConfig<'config>,
	{
		let config: AttachConfig<'_> = config();
		let exceptions_policy: AttachmentExceptionPolicy = config.exceptions_policy;
		let mut scope: ScopeToken = ScopeToken::default();
		// Safety: The ScopeToken and guard will remain fixed on the stack by keeping the guard
		// private to this function.
		let mut guard: AttachGuard<'_> = unsafe { self.attach_current_thread_guard(|| config, &mut scope)? };
		let env: &mut Env<'_> = guard.borrow_env_mut();

		let pre_pending_exception: Option<crate::objects::JThrowable<'_>> = match exceptions_policy {
			AttachmentExceptionPolicy::Ignore => None,
			AttachmentExceptionPolicy::PreCheckPostCatch => {
				// In case the thread was already attached we must not go any further if there is a pending exception,
				// otherwise this API would have the surprising side effect of clearing pending exceptions. In other
				// words we behave as if this API is not exception safe and immediately return `JavaException` to
				// indicate there is a pending exception that must be handled.
				if env.exception_check() {
					return Err(Error::JavaException.into());
				}
				None
			}
			AttachmentExceptionPolicy::PreReThrowPostCatch => env.exception_occurred().inspect(|_| {
				env.exception_clear();
			}),
		};

		let ret: std::result::Result<T, E> = match catch_unwind(AssertUnwindSafe(|| {
			if let Some(capacity) = capacity {
				env.with_local_frame(capacity, callback)
			} else {
				callback(env)
			}
		})) {
			Ok(ret) => ret,
			Err(payload) => {
				// The callback's local frame has unwound, but the attachment and saved
				// throwable are still live. Restore before AttachGuard drops on unwind.
				if let Some(throwable) = pre_pending_exception {
					env.exception_clear();
					drop(env.throw(throwable));
				}
				resume_unwind(payload);
			}
		};

		// Explicitly detach the guard (instead of relying on Drop) so we can
		// return any error from the detachment process.
		//
		// Any left-over pending exception from the `callback` closure will be
		// returned as `Error::CaughtJavaException` here.
		//
		// Any exception that was caught before running the `callback` closure
		// will be re-thrown.
		//
		// Note: any error here will also have a higher precedence than the
		// result from the closure which is probably reasonable because a
		// detachment error would probably indicate a serious issue and an
		// exception should typically imply that the closure has returned a
		// `JavaException` error.
		if exceptions_policy == AttachmentExceptionPolicy::Ignore {
			guard.detach()?;
		} else {
			guard.detach_with_catch_and_throw(pre_pending_exception)?;
		}

		ret
	}

	/// Attaches the current thread to the Java VM and returns an [`AttachGuard`] for the
	/// attachment.
	///
	/// This is a low-level (unsafe) building block for [`Self::attach_current_thread`],
	/// [`Self::attach_current_thread_for_scope`] and [`Self::attach_current_thread_with_config`]
	/// that allows for more fine-grained control over the attachment process and how you borrow an
	/// `Env` reference from the guard.
	///
	/// The given `config` callback is only lazily called if the thread was not already attached and
	/// returns a [`AttachConfig`] that you can use to customize the attachment.
	///
	/// For example, this can be used to implement your own equivalent to
	/// [`Self::attach_current_thread`] like:
	///
	/// ```rust,ignore-aarch64-linux-android
	/// # use abos_jni::{JavaVM, AttachConfig, DEFAULT_LOCAL_FRAME_CAPACITY};
	/// struct Executor {
	///     vm: JavaVM,
	/// }
	/// impl Executor {
	///     fn new(vm: JavaVM) -> Self {
	///         Self { vm }
	///     }
	///
	///     pub fn my_attach_current_thread<F, T, E>(&self, callback: F) -> std::result::Result<T, E>
	///     where
	///         F: FnOnce(&mut abos_jni::Env) -> std::result::Result<T, E>,
	///         E: From<abos_jni::errors::Error>,
	///     {
	///         let mut scope = abos_jni::ScopeToken::default();
	///         // Safety: The scope + guard will remain fixed on the stack by keeping the guard
	///         // private to this function.
	///         let mut guard = unsafe { self.vm.attach_current_thread_guard(AttachConfig::default, &mut scope)? };
	///         let ret = guard.borrow_env_mut().with_local_frame(DEFAULT_LOCAL_FRAME_CAPACITY, callback);
	///         guard.detach_with_catch()?; // return pending exceptions as Error::CaughtJavaException
	///         ret
	///     }
	/// }
	/// ```
	///
	/// See [`Self::attach_current_thread`], [`Self::attach_current_thread_for_scope`],
	/// [`AttachConfig`] and [`AttachGuard`] for more details.
	///
	/// Consider using [`Self::attach_current_thread_with_config`] before resorting to this (unsafe)
	/// API.
	///
	/// # Safety
	///
	/// The returned guard must be managed according to the general [`AttachGuard`] "Safety" rules.
	///
	/// **IMPORTANTLY:** Never give the returned guard a `'static` lifetime by creating a `'static`
	/// [`ScopeToken`]; the guard must be treated as an immovable value on the stack and not be
	/// re-ordered relative to other guards on the stack. See the 'Safety' rules for [`AttachGuard`]
	///
	/// Since the `'local` lifetime for the [Env] reference will come from the `&'scope ScopeToken`
	/// reference, you must ensure that the `scope` is dropped in sync with the guard (to ensure
	/// no local reference types may outlive the attachment).
	///
	/// This can be achieved by hiding the scope and guard from safe code, only exposing a temporary [`Env`]
	/// reference (as in the example above).
	///
	///
	/// # Errors
	///
	/// Returns an error if the thread cannot be attached or its JNI environment cannot be obtained.
	pub unsafe fn attach_current_thread_guard<'config, 'scope, F>(
		&self,
		config: F,
		_scope: &'scope mut ScopeToken,
	) -> Result<AttachGuard<'scope>>
	where
		F: FnOnce() -> AttachConfig<'config>,
	{
		// Safety:
		// - The minimum supported JNI version is >= 1.4
		// - The caller is responsible for managing the returned guard safely
		let guard: AttachGuard<'_> = unsafe {
			// Note: Even Windows (where we track permanent attachments via FLS) we use TLS to track
			// active AttachGuards for the current thread because we assume that fibers using JNI must
			// not be switched out or freed while they have active AttachGuards.
			if let Some(guard) = Self::try_get_nested_env_attach_guard::<'scope>() {
				guard
			} else {
				// If we don't have an active AttachGuard we _always_ check the real JNI GetEnv
				// state even if we have a permanent attachment recorded in TLS/FLS. We want to be
				// resilient to external code detaching the thread, so long as there are no active
				// AttachGuards.
				match self.sys_get_env_attachment() {
					Ok(guard) => guard,
					Err(Error::JniCall(JniError::ThreadDetached)) => {
						let config: AttachConfig<'_> = config();
						if config.scoped {
							let jni: *mut *const abos_jni_sys::JNINativeInterface_ =
								sys_attach_current_thread(self, &config, &current(), true)?;
							AttachGuard::from_owned(jni)
						} else {
							#[cfg(use_tls_attach_guard)]
							{
								tls_attach_guard::tls_attach_current_thread(self, &config)?
							}
							#[cfg(use_fls_attach_guard)]
							{
								fls_attach_guard::fls_attach_current_thread(self, &config)?
							}
						}
					}
					Err(err) => Err(err)?,
				}
			}
		};

		Ok(guard)
	}

	/// Explicitly detaches the current thread from the JVM, **IFF** it was previously attached
	/// using [`JavaVM::attach_current_thread`] **AND** if there is no [`AttachGuard`] also keeping
	/// the current thread attached (I.e. you have no [`Env`] reference in scope).
	///
	/// This will always return an error if there are currently any active [`AttachGuard`]s
	/// (detaching the thread in this case would effectively turn guards into invalid, dangling
	/// pointers).
	///
	/// Detaching a non-attached thread is a no-op that won't return an error (assuming there's no
	/// active [`AttachGuard`] as noted above).
	///
	/// This API has no effect on thread attachments that were created outside of this crate's API
	/// (such as other JNI language bindings).
	///
	/// This API is not applicable to scoped attachments because it's an error to call while there
	/// are active [`AttachGuard`]s.
	///
	/// **Windows Note**: On Windows, in the unlikely event that you have multiple fibers scheduled
	/// on the same thread, this API only has an effect within the context of a fiber that previously
	/// attached the thread to the JVM via [`JavaVM::attach_current_thread`].
	///
	/// _**Note**: It's _rarely_ necessary to use this API because a thread that is attached via
	/// [`JavaVM::attach_current_thread`] will automatically detach when that thread terminates.
	///
	/// Explicitly detaching the thread could lead to overheads later if the same thread needs to
	/// get re-attached.
	///
	/// If there is a need to detach a thread before it terminates, then it's possible that a scoped
	/// attachment via [`JavaVM::attach_current_thread_for_scope`] could be used so that the
	/// detachment would happen automatically.
	///
	/// # Errors
	///
	/// Returns an error if active attachment guards prevent detachment or JNI detachment fails.
	pub fn detach_current_thread(&self) -> Result<()> {
		if THREAD_GUARD_NEST_LEVEL.get() != 0 {
			return Err(Error::ThreadAttachmentGuarded);
		}

		#[cfg(use_tls_attach_guard)]
		{
			tls_attach_guard::tls_detach_current_thread()
		}

		#[cfg(use_fls_attach_guard)]
		{
			fls_attach_guard::fls_detach_current_thread()
		}
	}
}
