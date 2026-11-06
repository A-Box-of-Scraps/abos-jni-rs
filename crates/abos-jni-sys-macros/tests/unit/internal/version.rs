use super::JniVersion;

#[test]
fn versions_are_ordered_numerically_with_reserved_last() {
	let mut versions: Vec<JniVersion> = ["reserved", "24", "1.10", "1.2", "999"]
		.into_iter()
		.map(|version| syn::parse_str(&format!("\"{version}\"")).expect("valid version"))
		.collect();
	versions.sort();
	let suffixes: Vec<String> = versions.into_iter().map(JniVersion::suffix).collect();
	assert_eq!(suffixes, ["1_2", "1_10", "24", "999", "reserved"]);
}

#[test]
fn rejects_invalid_versions() {
	for version in ["", "1.", ".2", "1.2.3", "-1", "65536", "1.65536", "latest"] {
		assert!(
			syn::parse_str::<JniVersion>(&format!("\"{version}\"")).is_err(),
			"{version}"
		);
	}
}
