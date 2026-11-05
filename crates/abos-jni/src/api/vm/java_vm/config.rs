use super::*;

/// The policy for how exceptions are handled before and after running
/// an attachment closure.
///
/// This policy affects the closure-based thread attachment APIs, including:
/// - [`JavaVM::attach_current_thread`]
/// - [`JavaVM::attach_current_thread_for_scope`]
/// - [`JavaVM::attach_current_thread_with_config`]
///
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum AttachmentExceptionPolicy {
	/// Pending exceptions are not checked before or after any attachment
	/// closure
	///
	/// This will expose your closure to any pending exception which it should
	/// be prepared to handle.
	///
	/// For example this could be used to make purely exception-safe JNI calls.
	///
	/// Note: It should be very rare to use this, since attachment closures are
	/// normally written as orthogonal code that is not expected to handle the
	/// side effects of external code.
	Ignore,

	/// Pending exceptions are caught before running any attachment closure and
	/// re-thrown afterwards, returning [`Error::JavaException`]
	///
	/// If an exception is thrown by the attachment closure then this will
	/// _always_ be caught and returned via [`Error::CaughtJavaException`].
	///
	/// If a pending exception was caught **and** an exception is thrown by the
	/// attachment closure then:
	/// 1. The new exception (from the closure) will be caught
	/// 2. The original exception will be re-thrown
	/// 3. The new exception will be returned via [`Error::CaughtJavaException`]
	///
	/// Beware: that in this case the original exception will be pending after
	/// returning, but this will not be indicated via a [`Error::JavaException`]
	/// return value. The assumption in this case is that the caller is already
	/// aware of this pending exception.
	///
	/// This is intended to make the best default trade off between avoiding
	/// side effects and consistently running orthogonal code within an
	/// attachment closure.
	///
	/// This prioritizes consistency for the attachment closure:
	/// - It will *always run*, regardless of unexpected pending exceptions
	///   associated with the current thread.
	/// - Any new exception thrown by the closure will be *always be caught* and
	///   returned via [`Error::CaughtJavaException`] - regardless of whether
	///   some other exception was previously pending.
	///
	/// A pre-existing exception _may_ represent an oversight with handling an
	/// exception in code that is unrelated to the attachment closure, **or** it
	/// could also easily arise if `Drop` is used to release resources via JNI
	/// because these may automatically run while unwinding a stack (perhaps due
	/// to an [`Error::JavaException`] that the caller knows about).
	#[default]
	PreReThrowPostCatch,

	/// [`Error::JavaException`] is immediately returned if there is a pending
	/// exception and the attachment closure is not run.
	///
	/// If any exception is thrown by the attachment closure then this will be
	/// caught and returned via [`Error::CaughtJavaException`]
	///
	/// This is similar to the behaviour of other `jni` APIs that are not
	/// exception-safe and will immediately return with an error if called
	/// while there is a pending exception.
	PreCheckPostCatch,
}

/// Configuration options for attaching the current thread to a Java VM.
#[derive(Debug, Default)]
pub struct AttachConfig<'a> {
	pub(super) scoped: bool,
	pub(super) exceptions_policy: AttachmentExceptionPolicy,
	pub(super) deprecated_thread_name: Option<JNIString>,
	pub(super) thread_name: Option<&'a JNIStr>,
	pub(super) group: Option<&'a Global<JObject<'static>>>,
}

impl<'a> AttachConfig<'a> {
	/// Creates a new `AttachConfig` with default values.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets whether the attachment should be owned by the current scope, such
	/// that the thread will be automatically detached when the attachment guard
	/// is dropped.
	///
	/// The default is `false`, so the thread will be attached permanently.
	///
	/// It is normally best to attach permanently because it can reduce the cost
	/// of repeatedly attaching and detaching threads.
	pub fn scoped(mut self, scoped: bool) -> Self {
		self.scoped = scoped;
		self
	}

	/// Sets the policy for handling exceptions during thread attachment.
	///
	/// The default is [`AttachmentExceptionPolicy::PreReThrowPostCatch`] which
	/// is recommended to avoid the side effects of clearing (existing) pending
	/// exceptions and will return any new exception thrown by the closure via
	/// [`Error::CaughtJavaException`].
	///
	/// For example, the default policy should be suitable for code that needs
	/// to use JNI from within a `Drop` implementation, without being influenced
	/// by external pending exceptions but also ensuring that external pending
	/// exception continue to propagate.
	pub fn exceptions_policy(mut self, policy: AttachmentExceptionPolicy) -> Self {
		self.exceptions_policy = policy;
		self
	}

	/// Sets the name of the thread as seen by the JVM and operating system.
	#[deprecated(since = "0.22.2", note = "Use `thread_name` instead")]
	pub fn name<S: AsRef<str>>(mut self, name: S) -> Self {
		self.deprecated_thread_name = Some(JNIString::from(name.as_ref()));
		self
	}

	/// Sets the name of the thread as seen by the JVM and operating system.
	pub fn thread_name(mut self, name: &'a JNIStr) -> Self {
		self.thread_name = Some(name);
		self
	}

	/// Specifies a global reference to a `ThreadGroup` that the thread should
	/// be associated with.
	pub fn group(mut self, group: &'a Global<JObject<'static>>) -> Self {
		self.group = Some(group);
		self
	}
}
