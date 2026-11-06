//! Android generation must fail without publishing incomplete binding output.
use std::{
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Output},
};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Failure {
	Parse,
	Generation,
}

impl Failure {
	const fn name(self) -> &'static str {
		match self {
			Self::Parse => "parse",
			Self::Generation => "generation",
		}
	}
}

fn sdk(failure: Failure) -> TestResult<TempDir> {
	let root: TempDir = tempfile::tempdir()?;
	let sources: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java");
	let classes: PathBuf = root.path().join("classes");
	let fixture: PathBuf = root.path().join("UnsupportedNames.java");
	let mut source: String = fs::read_to_string(sources.join("UnsupportedNames.java"))?;
	if matches!(failure, Failure::Generation) {
		// Missing primitive mappings remain generation errors even when dollar names are supported.
		source = source.replacen(
			"public class UnsupportedNames {",
			"public class UnsupportedNames {
			public void unsupported(@io.github.jni_rs.jbindgen.RustPrimitive(\"Missing\") int value) {}",
			1,
		);
	}
	fs::write(&fixture, source)?;
	let mut java_sources: Vec<PathBuf> = abos_jbindgen_annotations::write_sources(root.path().join("annotations"))?;
	java_sources.extend([sources.join("SimpleClass.java"), fixture.clone()]);
	let compiled: Vec<PathBuf> = abos_javac::Build::new()
		.files(java_sources)
		.output_dir(&classes)
		.cargo_metadata(false)
		.try_compile()?;
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	let mut bytecode: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join("android.jar"))?);
	let mut stubs: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join("android-stubs-src.jar"))?);
	// The valid class sorts first, exercising failure after successful work on earlier classes.
	for name in ["SimpleClass", "UnsupportedNames"] {
		let entry: String = format!("com/example/{name}.class");
		let bytes: Vec<u8> = if matches!(failure, Failure::Parse) && name == "UnsupportedNames" {
			vec![0, 1, 2]
		} else {
			fs::read(classes.join(&entry))?
		};
		bytecode.start_file(entry, SimpleFileOptions::default())?;
		bytecode.write_all(&bytes)?;
		stubs.start_file(format!("com/example/{name}.java"), SimpleFileOptions::default())?;
		let source_path: PathBuf = if name == "UnsupportedNames" {
			fixture.clone()
		} else {
			sources.join(format!("{name}.java"))
		};
		stubs.write_all(&fs::read(source_path)?)?;
	}
	for class in compiled {
		let entry: &Path = class.strip_prefix(&classes)?;
		if entry.starts_with("io") {
			bytecode.start_file(entry.to_string_lossy(), SimpleFileOptions::default())?;
			bytecode.write_all(&fs::read(class)?)?;
		}
	}
	bytecode.finish()?;
	stubs.finish()?;
	Ok(root)
}

fn android(root: &Path) -> Command {
	let mut command = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"));
	command
		.args(["android", "--api-level", "35"])
		.env("ANDROID_HOME", root)
		.env_remove("ANDROID_SDK_ROOT");
	command
}

fn assert_failure(command: &mut Command, failure: Failure) -> TestResult<Output> {
	let output: Output = command.output()?;
	let stderr: String = String::from_utf8_lossy(&output.stderr).into_owned();
	assert!(
		!output.status.success(),
		"{} failure incorrectly succeeded: {stderr}",
		failure.name()
	);
	assert!(output.stdout.is_empty(), "partial bindings were written to stdout");
	assert!(stderr.contains("UnsupportedNames"), "missing class context: {stderr}");
	match failure {
		Failure::Parse => {
			assert!(stderr.contains("android.jar"), "missing archive context: {stderr}");
			assert!(
				stderr.contains("truncated class file"),
				"missing original error: {stderr}"
			);
		}
		Failure::Generation => {
			assert!(stderr.contains("Missing"), "missing original error: {stderr}");
			assert!(stderr.contains("primitive type mapping"), "missing cause: {stderr}");
		}
	}
	Ok(output)
}

#[test]
fn selected_class_failures_abort_instead_of_returning_partial_bindings() -> TestResult {
	for failure in [Failure::Parse, Failure::Generation] {
		let root: TempDir = sdk(failure)?;
		assert_failure(&mut android(root.path()), failure)?;
	}
	Ok(())
}

#[test]
fn library_error_probe() {
	use abos_jbindgen::{Builder, Error};
	use std::error::Error as _;
	let Ok(scenario) = std::env::var("ABOS_JBINDGEN_ANDROID_FAILURE") else {
		return;
	};
	let error: Error = Builder::new()
		.input_android_sdk(35, Vec::new())
		.generate()
		.expect_err("selected class failure");
	let cause: String = error.source().expect("original error source").to_string();
	match (scenario.as_str(), error) {
		("parse", Error::AndroidClassParse { archive, entry, source }) => {
			assert!(archive.ends_with("platforms/android-35/android.jar"));
			assert_eq!(entry, "com/example/UnsupportedNames.class");
			assert_eq!(cause, source.to_string());
			assert!(matches!(*source, Error::ParseClass(_)));
		}
		("generation", Error::AndroidClassGeneration { class_name, source }) => {
			assert_eq!(class_name, "com.example.UnsupportedNames");
			assert_eq!(cause, source.to_string());
			assert!(matches!(*source, Error::CodeGen(_)));
		}
		(_, error) => panic!("unexpected error for {scenario}: {error:?}"),
	}
}

#[test]
fn library_errors_preserve_typed_causes_and_context() -> TestResult {
	// Isolate SDK environment variables and JVM state in child processes.
	for failure in [Failure::Parse, Failure::Generation] {
		let root: TempDir = sdk(failure)?;
		let output: Output = Command::new(std::env::current_exe()?)
			.args(["--exact", "library_error_probe", "--nocapture"])
			.env("ANDROID_HOME", root.path())
			.env_remove("ANDROID_SDK_ROOT")
			.env("ABOS_JBINDGEN_ANDROID_FAILURE", failure.name())
			.output()?;
		assert!(
			output.status.success(),
			"{}{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}
	Ok(())
}

fn assert_outputs_untouched(root: &Path, failure: Failure, mode: &str, existing: bool) -> TestResult {
	let output_path: PathBuf = root.join(format!("bindings{mode}-{existing}"));
	let map_path: PathBuf = root.join(format!("types{mode}-{existing}.txt"));
	let code_path: PathBuf = if mode == "--output-dir" {
		output_path.join("mod.rs")
	} else {
		output_path.clone()
	};
	if existing {
		fs::create_dir_all(code_path.parent().expect("output parent"))?;
		fs::write(&code_path, "existing bindings\n")?;
		fs::write(&map_path, "existing type map\n")?;
	}
	let mut command: Command = android(root);
	command
		.arg(mode)
		.arg(&output_path)
		.arg("--output-type-map")
		.arg(&map_path);
	assert_failure(&mut command, failure)?;
	if existing {
		assert_eq!(fs::read_to_string(code_path)?, "existing bindings\n");
		assert_eq!(fs::read_to_string(map_path)?, "existing type map\n");
		if mode == "--output-dir" {
			assert_eq!(fs::read_dir(output_path)?.count(), 1);
		}
	} else {
		assert!(!output_path.exists(), "partial binding output was created");
		assert!(!map_path.exists(), "partial type map was created");
	}
	Ok(())
}

#[test]
fn failures_neither_create_nor_replace_output_files() -> TestResult {
	for failure in [Failure::Parse, Failure::Generation] {
		let root: TempDir = sdk(failure)?;
		for mode in ["--output", "--output-dir"] {
			for existing in [false, true] {
				assert_outputs_untouched(root.path(), failure, mode, existing)?;
			}
		}
	}
	Ok(())
}

#[test]
fn unselected_classes_do_not_abort_generation() -> TestResult {
	for failure in [Failure::Parse, Failure::Generation] {
		let root: TempDir = sdk(failure)?;
		let output: Output = android(root.path())
			.args(["--pattern", "com.example.SimpleClass"])
			.output()?;
		assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
		let code: String = String::from_utf8(output.stdout)?;
		assert!(code.contains("pub SimpleClass"));
		assert!(!code.contains("pub UnsupportedNames"));
	}
	Ok(())
}

#[test]
fn unsupported_members_require_explicit_skips_not_allow_unsupported() -> TestResult {
	let root: TempDir = sdk(Failure::Generation)?;
	assert_failure(android(root.path()).arg("--allow-unsupported"), Failure::Generation)?;
	let output: Output = android(root.path())
		.args(["--skip", "Lcom/example/UnsupportedNames;->unsupported(I)V"])
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	let code: String = String::from_utf8(output.stdout)?;
	assert!(code.contains("pub SimpleClass"));
	assert!(code.contains("pub UnsupportedNames"));
	assert!(code.contains("$value"));
	assert!(code.contains("foo$bar"));
	Ok(())
}
