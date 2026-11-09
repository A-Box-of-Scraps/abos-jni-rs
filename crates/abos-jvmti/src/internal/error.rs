use core::fmt;

use crate::{Error, Result, sys};

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::InvalidVersion(version) => write!(f, "invalid JVMTI version {version:#x}"),
			Self::UnsupportedVersion { required, actual } => write!(f, "JVMTI {required} required, got {actual}"),
			Self::Jni(code) => write!(f, "JNI invocation failed with code {code}"),
			Self::Jvmti(code) => write!(f, "JVMTI operation failed with code {code}"),
			Self::MissingFunction(name) => write!(f, "JVMTI function {name} is unavailable"),
			Self::NullPointer => f.write_str("null native interface or allocation pointer"),
			Self::InvalidCount(count) => write!(f, "negative JVMTI element count {count}"),
			Self::AllocationTooLarge => f.write_str("JVMTI allocation is too large"),
		}
	}
}

impl std::error::Error for Error {}

pub const fn check(code: sys::jvmtiError) -> Result<()> {
	if code == sys::JVMTI_ERROR_NONE {
		Ok(())
	} else {
		Err(Error::Jvmti(code))
	}
}
