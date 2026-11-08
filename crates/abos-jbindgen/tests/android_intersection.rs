//! Android callable signatures must match independently of the native modifier.
use std::{
	fs,
	io::Write,
	path::PathBuf,
	process::{Command, Output},
};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn sdk() -> TestResult<TempDir> {
	let root: TempDir = tempfile::tempdir()?;
	let fixtures: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/android/intersection");
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.file(fixtures.join("bytecode/CallableModifiers.java"))
		.output_dir(root.path().join("classes"))
		.cargo_metadata(false)
		.try_compile()?;
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	for (archive, entry, bytes) in [
		(
			"android.jar",
			"com/example/CallableModifiers.class",
			fs::read(&classes[0])?,
		),
		(
			"android-stubs-src.jar",
			"com/example/CallableModifiers.java",
			fs::read(fixtures.join("stubs/CallableModifiers.java"))?,
		),
	] {
		let mut writer: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join(archive))?);
		writer.start_file(entry, SimpleFileOptions::default())?;
		writer.write_all(&bytes)?;
		writer.finish()?;
	}
	Ok(root)
}

fn generate() -> TestResult<String> {
	let root: TempDir = sdk()?;
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.args([
			"android",
			"--api-level",
			"35",
			"--pattern",
			"com.example.CallableModifiers",
		])
		.env("ANDROID_HOME", root.path())
		.env_remove("ANDROID_SDK_ROOT")
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn intersects_callable_signatures_across_native_categories() -> TestResult {
	let code: String = generate()?;

	for name in [
		"native_value",
		"static_native_value",
		"ordinary_value",
		"static_ordinary_value",
		"matched_native",
		"matched_static_native",
		"matched_ordinary",
		"matched_static_ordinary",
	] {
		assert_eq!(code.matches(&format!("fn {name}(")).count(), 1, "{name}: {code}");
	}
	for name in [
		"static_native_value",
		"static_ordinary_value",
		"matched_static_native",
		"matched_static_ordinary",
	] {
		assert!(code.contains(&format!("static fn {name}(")), "{name}: {code}");
	}
	assert_eq!(code.matches("fn overloaded_").count(), 2, "{code}");
	assert!(code.contains("fn overloaded_int "), "{code}");
	assert!(code.contains("fn overloaded_string "), "{code}");
	assert!(!code.contains("fn overloaded_long"), "{code}");
	assert!(!code.contains("native_methods {"), "{code}");
	Ok(())
}

#[test]
fn excludes_missing_and_incompatible_signatures() -> TestResult {
	let code: String = generate()?;
	for name in [
		"wrong_parameter",
		"wrong_array",
		"wrong_return",
		"wrong_static",
		"wrong_instance",
		"jar_only",
		"stub_only",
	] {
		assert!(!code.contains(&format!("fn {name}(")), "{name}: {code}");
	}
	Ok(())
}

#[test]
fn preserves_metadata_from_both_stub_categories() -> TestResult {
	let code: String = generate()?;
	assert!(code.contains("Documentation from the non-native stub."), "{code}");
	assert!(code.contains("Documentation from the native stub."), "{code}");
	assert!(code.contains("#[deprecated]"), "{code}");
	assert!(code.contains("documented_name: jint"), "{code}");
	assert!(!code.contains("bytecode_name"), "{code}");
	Ok(())
}
