//! Build the example agent and exercise it in an isolated, bounded JVM process.
use std::{
	fs::{self, File},
	path::PathBuf,
	process::{Command, Output, Stdio},
	thread,
	time::{Duration, Instant},
};

#[test]
fn agent_receives_vm_and_thread_events() -> Result<(), Box<dyn std::error::Error>> {
	let manifest: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let target: PathBuf = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("events-agent");
	let build: Output = Command::new(env!("CARGO"))
		.args([
			"build",
			"--offline",
			"--locked",
			"--quiet",
			"--package",
			"abos-jvmti",
			"--example",
			"events_agent",
			"--target-dir",
		])
		.arg(&target)
		.output()?;
	assert!(
		build.status.success(),
		"agent build failed:\n{}",
		String::from_utf8_lossy(&build.stderr)
	);
	let agent: PathBuf = target.join("debug/examples").join(format!(
		"{}events_agent{}",
		std::env::consts::DLL_PREFIX,
		std::env::consts::DLL_SUFFIX
	));
	let home: PathBuf = PathBuf::from(std::env::var_os("JAVA_HOME").expect("Set JAVA_HOME to JDK 25"));
	let java: PathBuf = home.join("bin").join(format!("java{}", std::env::consts::EXE_SUFFIX));
	let directory: tempfile::TempDir = tempfile::tempdir()?;
	let log_path: PathBuf = directory.path().join("agent.log");
	let log: File = File::create(&log_path)?;
	// File-backed logs avoid pipe deadlocks and the temporary working directory
	// contains any JVM crash reports. The child timeout bounds native-agent failures.
	let mut child: std::process::Child = Command::new(java)
		.arg("-Xcheck:jni")
		.arg(format!("-agentpath:{}", agent.display()))
		.arg(manifest.join("tests/java/AgentSmoke.java"))
		.current_dir(directory.path())
		.stdout(Stdio::from(log.try_clone()?))
		.stderr(Stdio::from(log))
		.spawn()?;
	let start: Instant = Instant::now();
	let status: std::process::ExitStatus = loop {
		if let Some(status) = child.try_wait()? {
			break status;
		}
		if start.elapsed() > Duration::from_secs(60) {
			child.kill()?;
			child.wait()?;
			panic!("agent exceeded 60 seconds:\n{}", fs::read_to_string(&log_path)?);
		}
		thread::sleep(Duration::from_millis(20));
	};
	let output: String = fs::read_to_string(&log_path)?;
	assert!(status.success(), "JVM failed ({status}):\n{output}");
	for marker in [
		"JVMTI_VM_INIT version=",
		"JVMTI_WORKER_OK",
		"JVMTI_AGENT_OK thread_start=",
	] {
		assert!(output.contains(marker), "missing {marker}:\n{output}");
	}
	assert!(!output.contains("JVMTI_AGENT_ERROR"), "{output}");
	Ok(())
}
