//! Verify Cargo invalidates Java build output when tracked inputs change.

use std::{
	fs,
	path::Path,
	process::{Command, Output},
	thread,
	time::Duration,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(root: &Path, selection: &str) -> TestResult {
	let dependency: &str = env!("CARGO_MANIFEST_DIR");
	fs::create_dir_all(root.join("src"))?;
	fs::create_dir(root.join("java"))?;
	fs::write(root.join("src/lib.rs"), "")?;
	fs::write(
		root.join("Cargo.toml"),
		format!(
			"[package]\nname = \"cargo-rebuild-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
			[workspace]\n[build-dependencies]\nabos-javac = {{ path = {dependency:?} }}\n"
		),
	)?;
	fs::write(
		root.join("build.rs"),
		format!("fn main() {{ abos_javac::Build::new().{selection}.try_compile().expect(\"compile Java\"); }}"),
	)?;
	fs::write(root.join("java/Example.java"), "public class Example {}\n")?;
	Ok(())
}

fn build(root: &Path) -> std::io::Result<Output> {
	// A separate target directory avoids locking the parent Cargo test invocation.
	Command::new(env!("CARGO"))
		.args(["build", "--offline", "--quiet"])
		.current_dir(root)
		.env("CARGO_TARGET_DIR", root.join("target"))
		.output()
}

#[test]
fn cargo_rebuilds_after_source_edits_and_directory_additions() -> TestResult {
	for (selection, changed_file) in [
		("file(\"java/Example.java\")", "java/Example.java"),
		("source_dir(\"java\")", "java/Added.java"),
	] {
		let root: TempDir = tempfile::tempdir()?;
		fixture(root.path(), selection)?;
		let initial: Output = build(root.path())?;
		assert!(initial.status.success(), "{}", String::from_utf8_lossy(&initial.stderr));
		// Ensure the edit is newer than the build even on filesystems with coarse timestamps.
		thread::sleep(Duration::from_secs(2));
		fs::write(root.path().join(changed_file), "this is invalid Java\n")?;
		let rebuilt: Output = build(root.path())?;
		let stderr: String = String::from_utf8_lossy(&rebuilt.stderr).into_owned();
		assert!(
			!rebuilt.status.success(),
			"Cargo did not rebuild after changing {changed_file}"
		);
		assert!(stderr.contains("CompilationFailed"), "{stderr}");
	}
	Ok(())
}
