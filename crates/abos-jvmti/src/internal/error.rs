use core::fmt;

use crate::{Error, Result, sys};

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Jni(code) => write!(f, "JNI invocation failed with code {code}"),
			Self::Jvmti(code) => write!(f, "JVMTI operation failed with code {code}"),
			Self::MissingFunction(name) => write!(f, "JVMTI function {name} is unavailable"),
			Self::NullPointer => f.write_str("null native interface or allocation pointer"),
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
