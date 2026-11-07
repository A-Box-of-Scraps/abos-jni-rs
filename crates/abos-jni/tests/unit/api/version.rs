use super::JNIVersion;

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
