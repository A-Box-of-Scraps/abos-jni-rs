use abos_javac::{Build, Error, JavaCompiler};
use std::{
	env,
	path::PathBuf,
	process::{Command, Output},
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Isolate environment changes in child processes, without unsafe set_var or test races.
fn probe(case: &str) -> std::io::Result<Command> {
	let mut command = Command::new(env::current_exe()?);
	command
		.args(["--exact", "environment_probe", "--nocapture"])
		.env("ABOS_JAVAC_TEST_CASE", case)
		.env_remove("OUT_DIR");
	Ok(command)
}

fn assert_success(command: &mut Command) -> std::io::Result<Output> {
	let output: Output = command.output()?;
	assert!(
		output.status.success(),
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	Ok(output)
}

#[test]
fn environment_probe() -> TestResult {
	let Ok(case) = env::var("ABOS_JAVAC_TEST_CASE") else {
		return Ok(());
	};
	match case.as_str() {
		"missing-compiler" => {
			assert!(matches!(
				Build::new().try_get_compiler(),
				Err(Error::CompilerNotFound(_))
			));
		}
		"missing-output" => {
			assert!(matches!(Build::new().try_get_compiler(), Err(Error::OutDirNotSet)));
		}
		"cargo-output" => {
			let root: PathBuf = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
			let compiler: JavaCompiler = Build::new()
				.file("Foo.java")
				.output_subdir("nested")
				.try_get_compiler()?;
			let command: Command = compiler.to_command()?;
			let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
			for (flag, expected) in [
				("-d", root.join("javac-build/classes/nested")),
				("-s", root.join("javac-build/generated-sources")),
			] {
				let index: usize = args.iter().position(|arg| *arg == flag).expect("output flag");
				assert_eq!(args[index + 1], expected);
			}
		}
		"metadata" | "no-metadata" => {
			let compiler: JavaCompiler = Build::new()
				.file("Foo.java")
				.source_dir(env::var_os("OUT_DIR").expect("OUT_DIR"))
				.classpath("dependency.jar")
				.emit_rerun_if_changed(true)
				.cargo_metadata(case == "metadata")
				.try_get_compiler()?;
			// The missing input deliberately fails; metadata must still precede execution.
			assert!(compiler.try_compile().is_err());
		}
		"java-home" => {
			let compiler: JavaCompiler = Build::new().try_get_compiler()?;
			assert_eq!(
				compiler.path(),
				PathBuf::from(env::var_os("JAVA_HOME").expect("JAVA_HOME")).join("bin/javac")
			);
		}
		_ => panic!("unexpected test case: {case}"),
	}
	Ok(())
}

#[test]
fn reports_missing_compiler() -> TestResult {
	let mut command: Command = probe("missing-compiler")?;
	command.env_remove("JAVA_HOME").env("PATH", "");
	assert_success(&mut command)?;
	Ok(())
}

#[test]
fn requires_explicit_output_outside_cargo() -> TestResult {
	assert_success(&mut probe("missing-output")?)?;
	Ok(())
}

#[test]
fn resolves_cargo_output_defaults() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let mut command: Command = probe("cargo-output")?;
	command.env("OUT_DIR", root.path());
	assert_success(&mut command)?;
	Ok(())
}

#[test]
fn emits_metadata_only_when_enabled() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	for enabled in [true, false] {
		let mut command: Command = probe(if enabled { "metadata" } else { "no-metadata" })?;
		command.env("OUT_DIR", root.path());
		let output: Output = assert_success(&mut command)?;
		let stdout: String = String::from_utf8(output.stdout)?;
		assert_eq!(stdout.contains("cargo:"), enabled);
		if enabled {
			for directive in [
				"cargo:rerun-if-env-changed=JAVA_HOME",
				"cargo:rerun-if-env-changed=PATH",
				"cargo:rerun-if-changed=Foo.java",
				"cargo:rerun-if-changed=dependency.jar",
			] {
				assert!(stdout.contains(directive));
			}
			assert!(stdout.contains(&format!("cargo:rerun-if-changed={}", root.path().display())));
		}
	}
	Ok(())
}

#[cfg(unix)]
#[test]
fn prefers_java_home_over_path() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let compiler: JavaCompiler = Build::new().output_dir(root.path()).try_get_compiler()?;
	std::fs::create_dir(root.path().join("bin"))?;
	std::os::unix::fs::symlink(compiler.path(), root.path().join("bin/javac"))?;
	let mut command: Command = probe("java-home")?;
	command
		.env("JAVA_HOME", root.path())
		.env("OUT_DIR", root.path())
		.env("PATH", "");
	assert_success(&mut command)?;
	Ok(())
}
