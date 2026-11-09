//! Exercise the CLI through its installed binary interface.
use std::{
	fs,
	path::PathBuf,
	process::{Command, Output},
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn exports_annotations_and_reports_bad_arguments() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.args(["annotations", "--output"])
		.arg(root.path())
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert!(root.path().join("io/github/jni_rs/jbindgen/RustName.java").is_file());
	assert!(
		!Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
			.arg("unknown-command")
			.output()?
			.status
			.success()
	);
	Ok(())
}

#[test]
fn generates_source_bindings_and_type_map() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let java: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.arg("java")
		.arg(java)
		.args(["--root", "crate::model", "--output"])
		.arg(root.path().join("bindings.rs"))
		.arg("--output-type-map")
		.arg(root.path().join("types.txt"))
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert!(fs::read_to_string(root.path().join("bindings.rs"))?.contains("pub SimpleClass"));
	assert!(fs::read_to_string(root.path().join("types.txt"))?.contains("crate::model::com::example::SimpleClass"));
	Ok(())
}

#[test]
fn rejects_malformed_java_instead_of_generating_partial_bindings() -> TestResult {
	let java: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases/invalid/Broken.java");
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.arg("java")
		.arg(java)
		.output()?;
	assert!(!output.status.success());
	assert_eq!(output.stdout, [] as [u8; 0]);
	Ok(())
}

#[test]
fn rejects_missing_source_directory_without_output() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let missing: PathBuf = root.path().join("missing-directory");
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.arg("java")
		.arg(&missing)
		.output()?;
	assert!(!output.status.success());
	assert_eq!(output.stdout, [] as [u8; 0]);
	assert!(String::from_utf8_lossy(&output.stderr).contains(missing.to_str().ok_or("invalid path")?));
	Ok(())
}

#[test]
fn invalid_source_inputs_preserve_existing_outputs() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let java: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	let bindings: PathBuf = root.path().join("bindings.rs");
	let types: PathBuf = root.path().join("types.txt");
	fs::write(&bindings, "existing bindings")?;
	fs::write(&types, "existing type map")?;
	for name in ["unsupported.txt", "unsupported.class", "unsupported"] {
		fs::write(root.path().join(name), "not Java source")?;
	}
	for name in [
		"missing-directory",
		"Missing.java",
		"missing.jar",
		"unsupported.txt",
		"unsupported.class",
		"unsupported",
	] {
		let invalid: PathBuf = root.path().join(name);
		let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
			.arg("java")
			.arg(&java)
			.arg(&invalid)
			.arg("--output")
			.arg(&bindings)
			.arg("--output-type-map")
			.arg(&types)
			.output()?;
		assert!(!output.status.success(), "{name}");
		assert_eq!(output.stdout, [] as [u8; 0], "{name}");
		assert!(String::from_utf8_lossy(&output.stderr).contains(invalid.to_str().ok_or("invalid path")?));
		assert_eq!(fs::read_to_string(&bindings)?, "existing bindings", "{name}");
		assert_eq!(fs::read_to_string(&types)?, "existing type map", "{name}");
	}
	Ok(())
}
