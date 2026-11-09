use crate::{JvmtiVersion, sys};

/// Result of a JVMTI operation.
pub type Result<T> = core::result::Result<T, Error>;

/// A native error or an invalid interface response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
	/// An invalid interface-version identifier, such as a JNI version.
	InvalidVersion(sys::jint),
	/// The environment does not provide the required interface version.
	UnsupportedVersion {
		/// Minimum required version.
		required: JvmtiVersion,
		/// Version provided by the environment or supplied to a constructor.
		actual: JvmtiVersion,
	},
	/// A JNI invocation error, such as `JNI_EVERSION` from `GetEnv`.
	Jni(sys::jint),
	/// A JVMTI error code. Unknown and vendor-specific values are preserved.
	Jvmti(sys::jvmtiError),
	/// A required function pointer is absent from the interface.
	MissingFunction(&'static str),
	/// An environment, table, or nonempty allocation pointer was null.
	NullPointer,
	/// The requested allocation cannot be represented by a Rust slice or `jlong`.
	AllocationTooLarge,
}
