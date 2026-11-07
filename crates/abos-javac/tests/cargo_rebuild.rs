//! Verify Cargo invalidates Java build output when tracked inputs change.

use std::{
	fs::{self, File, FileTimes},
	path::{Path, PathBuf},
	process::{Command, Output},
	time::{Duration, SystemTime},
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
		indoc::formatdoc! {r#"
			[package]
			name = "cargo-rebuild-probe"
			version = "0.0.0"
			edition = "2024"
			[workspace]
			[build-dependencies]
			abos-javac = {{ path = {dependency:?} }}
		"#},
	)?;
	fs::write(
		root.join("build.rs"),
		indoc::formatdoc! {r#"
			fn main() {{
				std::fs::write("build-ran", b"").expect("record build script execution");
				abos_javac::Build::new().{selection}.try_compile().expect("compile Java");
			}}
		"#},
	)?;
	fs::write(root.join("java/Example.java"), "public class Example {}\n")?;
	Ok(())
}

fn build(root: &Path, target: &Path) -> std::io::Result<Output> {
	// A separate target directory avoids locking the parent Cargo test invocation.
	Command::new(env!("CARGO"))
		.args(["build", "--offline", "--quiet"])
		.current_dir(root)
		.env("CARGO_TARGET_DIR", target)
		.output()
}

#[test]
fn cargo_rebuilds_after_source_edits_and_directory_additions() -> TestResult {
	let workspace: TempDir = tempfile::tempdir()?;
	let target: PathBuf = workspace.path().join("target");
	for (name, selection, changed_file) in [
		("file", "file(\"java/Example.java\")", "java/Example.java"),
		("directory", "source_dir(\"java\")", "java/Added.java"),
	] {
		// Keep independent source trees, but compile their shared dependencies only once.
		let root: PathBuf = workspace.path().join(name);
		fixture(&root, selection)?;
		let initial: Output = build(&root, &target)?;
		assert!(initial.status.success(), "{}", String::from_utf8_lossy(&initial.stderr));

		let marker: PathBuf = root.join("build-ran");
		fs::remove_file(&marker)?;
		let unchanged: Output = build(&root, &target)?;
		assert!(
			unchanged.status.success(),
			"{}",
			String::from_utf8_lossy(&unchanged.stderr)
		);
		assert!(!marker.exists(), "unchanged {name} fixture reran its build script");

		let completed: SystemTime = SystemTime::now();
		let changed: PathBuf = root.join(changed_file);
		fs::write(&changed, "this is invalid Java\n")?;
		// A future mtime avoids sleeping and stays newer even on coarse-timestamp filesystems.
		// The added file was not an explicit input, so directory tracking must discover it.
		File::options()
			.write(true)
			.open(&changed)?
			.set_times(FileTimes::new().set_modified(completed + Duration::from_secs(60)))?;
		assert!(fs::metadata(&changed)?.modified()? > completed);

		let rebuilt: Output = build(&root, &target)?;
		let stderr: String = String::from_utf8_lossy(&rebuilt.stderr).into_owned();
		assert!(
			!rebuilt.status.success(),
			"Cargo did not rebuild after changing {changed_file}"
		);
		assert!(
			marker.exists(),
			"build script did not run after changing {changed_file}"
		);
		assert!(stderr.contains("CompilationFailed"), "{stderr}");
	}
	Ok(())
}
