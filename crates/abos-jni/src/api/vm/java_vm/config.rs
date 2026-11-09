#[cfg(doc)]
use crate::{errors::Error, vm::JavaVM};

use super::{Global, JNIStr, JNIString, JObject};

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

	/// Temporarily clears a pre-existing exception before running the attachment closure,
	/// then restores it afterwards without changing the closure's return value.
	///
	/// If the closure leaves a new exception pending, that exception is caught and returned
	/// as [`Error::CaughtJavaException`] instead of the closure's result. If both a saved
	/// exception and a new exception exist, the saved exception is restored and the new one
	/// is returned. The restored exception remains pending, but does not itself cause an
	/// [`Error::JavaException`] return value.
	///
	/// If the callback panics and unwinds, the saved exception is restored before the
	/// original Rust panic resumes, replacing any Java exception raised by the callback.
	///
	/// A pending exception alone does not prevent the callback from running; attachment
	/// and local-frame setup must still succeed. This is the default policy for independent
	/// JNI work, including cleanup in `Drop`, that must preserve an external exception
	/// while reporting any new exception left by the closure.
	#[default]
	PreReThrowPostCatch,

	/// [`Error::JavaException`] is immediately returned if there is a pending
	/// exception and the attachment closure is not run.
	///
	/// If the attachment closure leaves an exception pending, it is caught and
	/// returned via [`Error::CaughtJavaException`].
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
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets whether the attachment should be owned by the current scope, such
	/// that the thread will be automatically detached when the attachment guard
	/// is dropped.
	///
	/// The default is `false`, requesting a permanent attachment. This setting only
	/// applies when attaching a detached thread; an existing attachment is not changed.
	///
	/// It is normally best to attach permanently because it can reduce the cost
	/// of repeatedly attaching and detaching threads.
	#[must_use]
	pub const fn scoped(mut self, scoped: bool) -> Self {
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
	#[must_use]
	pub const fn exceptions_policy(mut self, policy: AttachmentExceptionPolicy) -> Self {
		self.exceptions_policy = policy;
		self
	}

	/// Sets the name of the thread as seen by the JVM and operating system.
	#[deprecated(since = "0.22.2", note = "Use `thread_name` instead")]
	#[must_use]
	pub fn name<S: AsRef<str>>(mut self, name: S) -> Self {
		self.deprecated_thread_name = Some(JNIString::from(name.as_ref()));
		self
	}

	/// Sets the name of the thread as seen by the JVM and operating system.
	#[must_use]
	pub const fn thread_name(mut self, name: &'a JNIStr) -> Self {
		self.thread_name = Some(name);
		self
	}

	/// Specifies a global reference to a `ThreadGroup` that the thread should
	/// be associated with.
	#[must_use]
	pub const fn group(mut self, group: &'a Global<JObject<'static>>) -> Self {
		self.group = Some(group);
		self
	}
}
