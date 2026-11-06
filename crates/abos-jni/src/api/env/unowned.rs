#[cfg(doc)]
use crate::errors::{LogContextErrorAndDefault, LogErrorAndDefault, ThrowRuntimeExAndDefault};

use super::{AssertUnwindSafe, AttachGuard, Env, Error, ErrorPolicy, catch_unwind};

/// The outcome of an operation that may succeed, fail with an error, or panic.
///
/// This enum is used to encapsulate the result of operations within native methods
/// where extra care is needed to handle errors and panics gracefully before
/// returning a value to the Java environment.
#[derive(Debug)]
pub enum Outcome<T, E> {
	/// Contains the success value
	Ok(T),
	/// Contains the error value
	Err(E),
	/// Contains the panic value
	Panic(Box<dyn std::any::Any + Send + 'static>),
}

/// An opaque wrapper around an [`Outcome<T, Error>`] that supports mapping
/// errors within native methods (via [`ErrorPolicy`]), with access to an
/// [`Env`] reference.
///
/// This is returned by [`EnvUnowned::with_env`] and is designed for use within
/// native method implementations where unwinding can't be caught by the JVM and
/// will abort the process.
///
/// Once you have an [`EnvOutcome`] you must resolve it to a value that can be
/// returned to the Java environment.
///
/// An [`EnvOutcome`] is resolved into a return value by calling
/// [`EnvOutcome::resolve`] or [`EnvOutcome::resolve_with`] with the help of an
/// [`ErrorPolicy`] that can customize how errors and panics are handled before
/// returning a value.
///
/// There are several built-in error policies in the `abos_jni::errors` module, and
/// you can also implement your own by implementing the [`ErrorPolicy`] trait.
/// See:
///
/// - [`ThrowRuntimeExAndDefault`]: an `ErrorPolicy` that throws any error as a
///   `java.lang.RuntimeException` and returns a default value.
/// - [`LogErrorAndDefault`]: an `ErrorPolicy` that logs errors and returns a
///   default value.
/// - [`LogContextErrorAndDefault`]: an `ErrorPolicy` that logs errors, with a
///   given context string, and returns a default value.
///
/// # Examples
///
/// ## Map Rust errors and panics to Java exceptions:
/// ```rust,no_run
/// # use abos_jni::objects::{JObject, JString};
/// # use abos_jni::errors::ThrowRuntimeExAndDefault;
/// #[unsafe(no_mangle)]
/// pub extern "system" fn Java_com_example_MyClass_myNativeMethod<'caller>(
///     mut unowned_env: abos_jni::EnvUnowned<'caller>,
///     _this: JObject<'caller>,
///     arg: JString<'caller>,
/// ) -> JObject<'caller> {
///     unowned_env.with_env(|env| -> abos_jni::errors::Result<_> {
///         // Use `env` to call Java methods or access fields.
///         Ok(JObject::null())
///     }).resolve::<ThrowRuntimeExAndDefault>()
/// }
/// ```
///
/// ## Log errors with a context string and return a default value:
/// ```rust,no_run
/// # use abos_jni::objects::{JObject, JString};
/// # use abos_jni::errors::LogContextErrorAndDefault;
/// #[unsafe(no_mangle)]
/// pub extern "system" fn Java_com_example_MyClass_myNativeMethod<'caller>(
///    mut unowned_env: abos_jni::EnvUnowned<'caller>,
///    _this: JObject<'caller>,
///    arg: JString<'caller>,
/// ) -> JObject<'caller> {
///    unowned_env.with_env(|env| -> abos_jni::errors::Result<_> {
///       // Use `env` to call Java methods or access fields.
///       Ok(JObject::null())
///   }).resolve_with::<LogContextErrorAndDefault, _>(|| {
///     format!("in myNativeMethod with arg: {arg}")
///   })
/// }
/// ```
#[must_use = "The outcome must be resolved to a value that can be returned to Java. See ::resolve or ::resolve_with"]
#[derive(Debug)]
pub struct EnvOutcome<'local, T, E> {
	raw_env: *mut crate::sys::JNIEnv,
	outcome: Outcome<T, E>,
	_invariant: std::marker::PhantomData<&'local mut ()>, // !Send/!Sync + tie to frame
}

impl<'local, T, E> EnvOutcome<'local, T, E> {
	pub(crate) const fn new(raw_env: *mut crate::sys::JNIEnv, outcome: Outcome<T, E>) -> Self {
		Self {
			raw_env,
			outcome,
			_invariant: std::marker::PhantomData,
		}
	}

	/// No captures (fast path).
	pub fn resolve<'native_method, P>(self) -> T
	where
		P: ErrorPolicy<T, E, Captures<'local, 'native_method> = ()>,
		T: Default + 'native_method,
		'local: 'native_method,
	{
		self.resolve_with::<P, _>(|| ())
	}

	/// Builder can borrow locals (via 'm) and use the temporary Env<'cf>.
	pub fn resolve_with<'native_method, P, F>(self, capture: F) -> T
	where
		P: ErrorPolicy<T, E>,
		T: Default + 'native_method,
		F: FnOnce() -> <P as ErrorPolicy<T, E>>::Captures<'local, 'native_method>,
		'local: 'native_method,
	{
		// Rebuild Env<'cf> (AttachGuard hidden), then build captures and map once.
		// All calls are guarded with catch_unwind; fall back to last_resort on failure.
		self.resolve_inner::<P, F>(capture)
	}

	fn resolve_inner<'native_method, P, F>(self, capture: F) -> T
	where
		P: ErrorPolicy<T, E>,
		T: Default + 'native_method,
		F: FnOnce() -> <P as ErrorPolicy<T, E>>::Captures<'local, 'native_method>,
		'local: 'native_method,
	{
		// SAFETY: EnvOutcome retains the native call's environment and lifetime, and the nested guard drops before returning.
		unsafe {
			let mut guard: AttachGuard<'local> = AttachGuard::from_unowned(self.raw_env);
			let env: &mut Env<'_> = guard.borrow_env_mut();

			match self.outcome {
				Outcome::Ok(t) => t,
				Outcome::Err(e) => {
					let mut cap: <P as ErrorPolicy<T, E>>::Captures<'_, '_> = capture();
					std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match P::on_error(env, &mut cap, e) {
						Ok(t) => t,
						Err(err) => P::on_internal_jni_error(&mut cap, err),
					}))
					.unwrap_or_else(|payload| P::on_internal_panic(&mut cap, payload))
				}
				Outcome::Panic(p) => {
					let mut cap: <P as ErrorPolicy<T, E>>::Captures<'_, '_> = capture();
					std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match P::on_panic(env, &mut cap, p) {
						Ok(t) => t,
						Err(err) => P::on_internal_jni_error(&mut cap, err),
					}))
					.unwrap_or_else(|payload| P::on_internal_panic(&mut cap, payload))
				}
			}
		}
	}

	/// Consumes the `EnvOutcome`, returning the underlying `Outcome<T, E>`.
	pub fn into_outcome(self) -> Outcome<T, E> {
		self.outcome
	}
}

/// Represents an external (unowned) JNI stack frame and thread attachment that
/// was passed to a native method call.
///
/// This is an FFI safe wrapper around a [`crate::sys::JNIEnv`] pointer that has
/// been passed as the first argument to a native method call, and represents
/// an implicit JNI thread attachment.
///
/// For example, you can use it with a native method implementation like this:
/// ```rust,no_run
/// # use abos_jni::objects::{JObject, JString};
/// # use abos_jni::errors::ThrowRuntimeExAndDefault;
/// #[unsafe(no_mangle)]
/// pub extern "system" fn Java_com_example_MyClass_myNativeMethod<'caller>(
///     mut unowned_env: abos_jni::EnvUnowned<'caller>,
///     _this: JObject<'caller>,
///     arg: JString<'caller>,
/// ) -> JObject<'caller> {
///     unowned_env.with_env(|env| -> abos_jni::errors::Result<_> {
///         // Use `env` to call Java methods or access fields.
///         Ok(JObject::null())
///     }).resolve::<ThrowRuntimeExAndDefault>()
/// }
/// ```
#[repr(transparent)]
#[derive(Debug)]
pub struct EnvUnowned<'local> {
	ptr: *mut abos_jni_sys::JNIEnv,
	_lifetime: std::marker::PhantomData<&'local ()>,
}

impl<'local> EnvUnowned<'local> {
	/// Runs a closure with a [`Env`] based on an unowned JNI thread attachment
	/// associated with an external JNI stack frame.
	///
	/// This API is specifically intended to be used within native/foreign Java
	/// method implementations in cases where you have named the lifetime for
	/// the caller's JNI stack frame.
	///
	/// It returns an [`EnvOutcome`] that supports mapping errors with access to
	/// an [`Env`], so you may choose to throw errors as exceptions.
	///
	/// To avoid the risk of unwinding into the JVM (which will abort the
	/// process) this API wraps the closure in a [`catch_unwind`] to catch any
	/// panics. Any panic can be handled via the [`ErrorPolicy`] given to
	/// [`EnvOutcome::resolve`] or handled directly via [`EnvOutcome::into_outcome`].
	///
	/// Note: This API does not create a new JNI stack frame, since the JVM will
	/// clean up the JNI stack frame when the native method returns.
	///
	/// Note: This API returns an [`EnvOutcome`]
	pub fn with_env<F, T, E>(&mut self, f: F) -> EnvOutcome<'local, T, E>
	where
		F: FnOnce(&mut Env<'local>) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		// Safety: we trust that self.ptr a valid, non-null pointer
		let mut guard: AttachGuard<'local> = unsafe { AttachGuard::from_unowned(self.ptr) };
		let env: &mut Env<'_> = guard.borrow_env_mut();
		let result: std::result::Result<std::result::Result<T, E>, Box<dyn std::any::Any + Send>> =
			catch_unwind(AssertUnwindSafe(|| f(env)));
		let outcome: Outcome<T, E> = match result {
			Ok(ret) => match ret {
				Ok(t) => Outcome::Ok(t),
				Err(e) => Outcome::Err(e),
			},
			Err(payload) => Outcome::Panic(payload),
		};
		EnvOutcome::new(self.ptr, outcome)
	}

	/// Runs a closure with a [`Env`] based on an unowned JNI thread attachment
	/// associated with an external JNI stack frame.
	///
	/// This API is specifically intended to be used within native/foreign Java
	/// method implementations in cases where you have named the lifetime for
	/// the caller's JNI stack frame.
	///
	/// Since it would lead to undefined behaviour to allow Rust code to unwind
	/// across a native method call boundary, you probably want to use
	/// [`EnvUnowned::with_env`] instead, which will wrap the closure
	/// in a `catch_unwind` to catch any panics.
	///
	/// Note: This API does not create a new JNI stack frame, which is normally
	/// what you want when implementing a native method, since the JVM will
	/// clean up the JNI stack frame when the native method returns.
	pub fn with_env_no_catch<F, T, E>(&mut self, f: F) -> EnvOutcome<'local, T, E>
	where
		F: FnOnce(&mut Env<'local>) -> std::result::Result<T, E>,
		E: From<Error>,
	{
		// Safety: we trust that self.ptr a valid, non-null pointer
		let mut guard: AttachGuard<'local> = unsafe { AttachGuard::from_unowned(self.ptr) };
		let result: std::result::Result<T, E> = f(guard.borrow_env_mut());
		match result {
			Ok(t) => EnvOutcome::new(self.ptr, Outcome::Ok(t)),
			Err(e) => EnvOutcome::new(self.ptr, Outcome::Err(e)),
		}
	}

	/// Creates a new `EnvUnowned` from a raw [`crate::sys::JNIEnv`] pointer.
	///
	/// It should be very uncommon to use this method directly, but could be
	/// useful if you are given a raw [`crate::sys::JNIEnv`] pointer that you
	/// know represents a valid JNI attachment for the current thread.
	///
	/// If you are implementing a native method in Rust though, you should
	/// prefer to use the [`EnvUnowned`] type as the first argument to your
	/// native method and avoid the need to use a raw pointer.
	///
	/// If you have a raw [`crate::sys::JNIEnv`] pointer, this API should be
	/// marginally safer than using [`crate::AttachGuard`] manually
	/// since the attach guard management will be hidden within the
	/// [`Self::with_env`] and [`Self::with_env_no_catch`] methods.
	///
	/// Beware that [`Self::with_env`] and [`Self::with_env_no_catch`] will not
	/// create a new JNI stack frame, so if you are not implementing a native
	/// method with a JNI stack frame that will be cleaned up on return, you may
	/// need to consider the risk of leaking local references into the current
	/// stack frame (JNI references are only cleaned up when the stack frame is
	/// popped)
	///
	/// # Safety
	///
	/// The pointer must be a valid, non-null pointer to a `abos_jni_sys::JNIEnv`
	/// that represents an attachment of the current thread to a Java VM.
	///
	/// The assigned lifetime must not outlive the JNI stack frame that owns the
	/// `Env` pointer. For example it would _never_ be safe to use `'static`.
	///
	/// # Panics
	///
	/// Panics if the raw pointer is null.
	pub unsafe fn from_raw(ptr: *mut abos_jni_sys::JNIEnv) -> Self {
		assert!(!ptr.is_null(), "EnvUnowned pointer must not be null");
		Self {
			ptr,
			_lifetime: std::marker::PhantomData,
		}
	}

	/// Returns the raw pointer to the underlying [`crate::sys::JNIEnv`].
	#[must_use]
	pub const fn as_raw(&self) -> *mut abos_jni_sys::JNIEnv {
		self.ptr
	}

	/// Consumes the [`EnvUnowned`] and returns the raw pointer to the underlying [`crate::sys::JNIEnv`].
	#[must_use]
	pub const fn into_raw(self) -> *mut abos_jni_sys::JNIEnv {
		self.ptr
	}
}
