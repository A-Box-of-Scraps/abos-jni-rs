use std::fmt;

/// Failures are propagated as Java exceptions by the generated native wrappers.
#[derive(Debug)]
pub enum TestError {
	Jni(abos_jni::errors::Error),
	Assertion(String),
}

impl From<abos_jni::errors::Error> for TestError {
	fn from(error: abos_jni::errors::Error) -> Self {
		Self::Jni(error)
	}
}

impl fmt::Display for TestError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Jni(error) => error.fmt(formatter),
			Self::Assertion(message) => formatter.write_str(message),
		}
	}
}

impl std::error::Error for TestError {}

pub type TestResult = Result<String, TestError>;

#[cfg(any(
	feature = "sdk_net_uri",
	feature = "sdk_os_build",
	feature = "sdk_content_intent",
	feature = "sdk_util_time_utils"
))]
pub fn require(condition: bool, message: impl Into<String>) -> Result<(), TestError> {
	if condition {
		Ok(())
	} else {
		Err(TestError::Assertion(message.into()))
	}
}

#[cfg(all(test, feature = "sdk_net_uri"))]
mod tests {
	use super::{TestError, require};

	#[test]
	fn assertions_return_failures_instead_of_panicking() {
		assert!(require(true, "passed").is_ok());
		let error = require(false, "unexpected URI host").expect_err("assertion failed");
		assert!(matches!(error, TestError::Assertion(_)));
		assert_eq!(error.to_string(), "unexpected URI host");
	}
}
