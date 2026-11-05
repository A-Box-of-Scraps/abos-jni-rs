//! Compile and execute generated bindings in a standalone Cargo project.
use std::{
	path::PathBuf,
	process::{Command, Output},
};

#[test]
fn generated_bindings_compile_and_execute() -> Result<(), Box<dyn std::error::Error>> {
	let manifest: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/runtime/Cargo.toml");
	let target: PathBuf = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("runtime");
	let output: Output = Command::new(env!("CARGO"))
		.args(["run", "--offline", "--locked", "--quiet", "--manifest-path"])
		.arg(manifest)
		.arg("--target-dir")
		.arg(target)
		.output()?;
	assert!(
		output.status.success(),
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	Ok(())
}
