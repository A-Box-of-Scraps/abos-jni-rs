use crate::{Error, Result, sys};

/// A validated JVMTI interface version, ordered by major, minor, and micro version.
///
/// JNI version identifiers are not interchangeable with JVMTI identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct JvmtiVersion(sys::jint);

impl JvmtiVersion {
	/// JVMTI 1.2, the minimum supported interface (available on Java 8).
	pub const V1_2: Self = Self(sys::JVMTI_VERSION_1_2);
	/// JVMTI 9, including module operations and early instrumentation capabilities.
	pub const V9: Self = Self(sys::JVMTI_VERSION_9);
	/// JVMTI 11, including sampled object allocation.
	pub const V11: Self = Self(sys::JVMTI_VERSION_11);
	/// JVMTI 19.
	pub const V19: Self = Self(sys::JVMTI_VERSION_19);
	/// JVMTI 21, including virtual-thread operations and events.
	pub const V21: Self = Self(sys::JVMTI_VERSION_21);
	/// JVMTI 25, the newest interface described by these bindings.
	pub const V25: Self = Self(sys::JVMTI_VERSION);
	/// Minimum interface required by the wrapper and requested by default.
	pub const MINIMUM: Self = Self::V1_2;

	/// Validates a raw JVMTI version, retaining unknown future versions.
	///
	/// # Errors
	/// Returns [`Error::InvalidVersion`] for non-JVMTI identifiers or reserved bits.
	/// Older valid versions are accepted here, but rejected by environment constructors.
	pub const fn from_raw(raw: sys::jint) -> Result<Self> {
		if raw < 0 || raw & sys::JVMTI_VERSION_MASK_INTERFACE_TYPE != sys::JVMTI_VERSION_INTERFACE_JVMTI {
			Err(Error::InvalidVersion(raw))
		} else {
			Ok(Self(raw))
		}
	}

	/// Returns the identifier accepted by `JavaVM::GetEnv`.
	#[must_use]
	pub const fn as_raw(self) -> sys::jint {
		self.0
	}

	/// Returns the major version component.
	#[must_use]
	pub const fn major(self) -> sys::jint {
		(self.0 & sys::JVMTI_VERSION_MASK_MAJOR) >> sys::JVMTI_VERSION_SHIFT_MAJOR
	}

	/// Returns the minor version component.
	#[must_use]
	pub const fn minor(self) -> sys::jint {
		(self.0 & sys::JVMTI_VERSION_MASK_MINOR) >> sys::JVMTI_VERSION_SHIFT_MINOR
	}

	/// Returns the micro version component.
	#[must_use]
	pub const fn micro(self) -> sys::jint {
		(self.0 & sys::JVMTI_VERSION_MASK_MICRO) >> sys::JVMTI_VERSION_SHIFT_MICRO
	}
}

impl TryFrom<sys::jint> for JvmtiVersion {
	type Error = Error;
	fn try_from(raw: sys::jint) -> Result<Self> {
		Self::from_raw(raw)
	}
}

impl From<JvmtiVersion> for sys::jint {
	fn from(version: JvmtiVersion) -> Self {
		version.as_raw()
	}
}

impl core::fmt::Display for JvmtiVersion {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{}.{}.{}", self.major(), self.minor(), self.micro())
	}
}
