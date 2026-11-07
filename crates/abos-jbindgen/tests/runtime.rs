//! Compile once, then execute generated bindings in isolated, bounded JVM processes.
use std::{
	fs::{self, File},
	path::{Path, PathBuf},
	process::{Command, Output, Stdio},
	thread,
	time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn generated_bindings_compile_and_execute() -> TestResult {
	let manifest: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/runtime/Cargo.toml");
	let target: PathBuf = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("runtime");
	let output: Output = Command::new(env!("CARGO"))
		.args(["build", "--offline", "--locked", "--quiet", "--manifest-path"])
		.arg(manifest)
		.arg("--target-dir")
		.arg(&target)
		.output()?;
	assert!(
		output.status.success(),
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	let executable: PathBuf = target
		.join("debug")
		.join(format!("abos-jbindgen-runtime-test{}", std::env::consts::EXE_SUFFIX));
	for scenario in [
		"existing",
		"generics",
		"enclosing",
		"records",
		"enums",
		"members",
		"inheritance",
		"identifiers",
	] {
		run_scenario(&executable, scenario)?;
	}
	Ok(())
}

fn run_scenario(executable: &Path, scenario: &str) -> TestResult {
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let log_path: PathBuf = output.path().join("runtime.log");
	let log: File = File::create(&log_path)?;
	// A file avoids pipe-buffer deadlocks, and also keeps JVM crash reports in the temp directory.
	let mut child: std::process::Child = Command::new(executable)
		.arg(scenario)
		.current_dir(output.path())
		.stdout(Stdio::from(log.try_clone()?))
		.stderr(Stdio::from(log))
		.spawn()?;
	let start: Instant = Instant::now();
	loop {
		if let Some(status) = child.try_wait()? {
			assert!(
				status.success(),
				"runtime scenario {scenario} ({status}):\n{}",
				fs::read_to_string(&log_path)?
			);
			return Ok(());
		}
		if start.elapsed() >= Duration::from_secs(120) {
			child.kill()?;
			child.wait()?;
			panic!(
				"runtime scenario {scenario} exceeded 120 seconds:\n{}",
				fs::read_to_string(&log_path)?
			);
		}
		thread::sleep(Duration::from_millis(20));
	}
}
