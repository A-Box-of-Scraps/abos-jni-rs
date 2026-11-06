/// JNI Version
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(transparent)]
pub struct JNIVersion {
	ver: u32,
}

impl JNIVersion {
	/// JNI Version 1.1
	pub const V1_1: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_1_1 as u32,
	};
	/// JNI Version 1.2
	pub const V1_2: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_1_2 as u32,
	};
	/// JNI Version 1.4
	pub const V1_4: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_1_4 as u32,
	};
	/// JNI Version 1.6
	pub const V1_6: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_1_6 as u32,
	};
	/// JNI Version 1.8
	pub const V1_8: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_1_8 as u32,
	};
	/// JNI Version 9.0
	pub const V9: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_9 as u32,
	};
	/// JNI Version 10.0
	pub const V10: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_10 as u32,
	};
	/// JNI Version 19.0
	pub const V19: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_19 as u32,
	};
	/// JNI Version 20.0
	pub const V20: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_20 as u32,
	};
	/// JNI Version 21.0
	pub const V21: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_21 as u32,
	};
	/// JNI Version 24.0
	pub const V24: Self = Self {
		ver: abos_jni_sys::JNI_VERSION_24 as u32,
	};

	/// Return a version from a raw version constant like [`abos_jni_sys::JNI_VERSION_1_2`]
	#[must_use]
	pub fn new(ver: abos_jni_sys::jint) -> Self {
		Self::from(ver)
	}

	/// Get the major component of the version number
	#[must_use]
	pub const fn major(&self) -> u16 {
		((self.ver & 0x00ff_0000) >> 16) as u16
	}

	/// Get the minor component of the version number
	#[must_use]
	pub const fn minor(&self) -> u16 {
		(self.ver & 0xff) as u16
	}
}

impl From<abos_jni_sys::jint> for JNIVersion {
	fn from(value: abos_jni_sys::jint) -> Self {
		Self {
			ver: value.cast_unsigned(),
		}
	}
}

impl From<JNIVersion> for abos_jni_sys::jint {
	fn from(val: JNIVersion) -> Self {
		val.ver.cast_signed()
	}
}

#[test]
fn jni_version_major_minor() {
	let cases: [(u16, u16, JNIVersion, i32); 10] = [
		(1, 1, JNIVersion::V1_1, abos_jni_sys::JNI_VERSION_1_1),
		(1, 2, JNIVersion::V1_2, abos_jni_sys::JNI_VERSION_1_2),
		(1, 4, JNIVersion::V1_4, abos_jni_sys::JNI_VERSION_1_4),
		(1, 6, JNIVersion::V1_6, abos_jni_sys::JNI_VERSION_1_6),
		(1, 8, JNIVersion::V1_8, abos_jni_sys::JNI_VERSION_1_8),
		(9, 0, JNIVersion::V9, abos_jni_sys::JNI_VERSION_9),
		(10, 0, JNIVersion::V10, abos_jni_sys::JNI_VERSION_10),
		(19, 0, JNIVersion::V19, abos_jni_sys::JNI_VERSION_19),
		(20, 0, JNIVersion::V20, abos_jni_sys::JNI_VERSION_20),
		(21, 0, JNIVersion::V21, abos_jni_sys::JNI_VERSION_21),
	];
	for (major, minor, version, raw) in cases {
		assert_eq!((version.major(), version.minor()), (major, minor));
		let reconstructed = JNIVersion::new(raw);
		assert_eq!((reconstructed.major(), reconstructed.minor()), (major, minor));
	}
}
