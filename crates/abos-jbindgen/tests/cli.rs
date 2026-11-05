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
