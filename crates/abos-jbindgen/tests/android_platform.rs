//! Android compilation uses SDK core classes and tolerates documentation-only metadata.
#[path = "support/android_platform.rs"]
mod android_platform;

use std::{
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Output},
};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn fixture(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join(name)
}

fn sdk(stub: &Path) -> TestResult<TempDir> {
	let root: TempDir = tempfile::tempdir()?;
	let classes_dir: PathBuf = root.path().join("classes");
	let mut sources: Vec<PathBuf> = abos_jbindgen_annotations::write_sources(root.path().join("annotations"))?;
	sources.push(fixture("java/android/platform/bytecode/PlatformProbe.java"));
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.files(sources)
		.release("8")
		.output_dir(&classes_dir)
		.cargo_metadata(false)
		.try_compile()?;
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	let mut bytecode: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join("android.jar"))?);
	android_platform::add_core_classes(&mut bytecode)?;
	for class in classes {
		let name: String = class.strip_prefix(&classes_dir)?.to_string_lossy().replace('\\', "/");
		bytecode.start_file(name, SimpleFileOptions::default())?;
		bytecode.write_all(&fs::read(class)?)?;
	}
	bytecode.finish()?;
	let mut stubs: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join("android-stubs-src.jar"))?);
	for (name, source) in [
		("com/example/PlatformProbe.java", stub.to_path_buf()),
		// Unselected java.* stubs must not conflict with host named modules.
		(
			"java/awt/font/AndroidFont.java",
			fixture("java/android/platform/AndroidFont.java"),
		),
	] {
		stubs.start_file(name, SimpleFileOptions::default())?;
		stubs.write_all(&fs::read(source)?)?;
	}
	stubs.finish()?;
	Ok(root)
}

fn generate(root: &Path) -> TestResult<Output> {
	Ok(Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.args(["android", "--api-level", "35", "--pattern", "com.example.PlatformProbe"])
		.env("ANDROID_HOME", root)
		.env_remove("ANDROID_SDK_ROOT")
		.output()?)
}

#[test]
fn handles_platform_packages_and_documentation_only_annotations() -> TestResult {
	// Keep the unused hidden import in an exact-input fixture, outside Java formatting.
	let root: TempDir = sdk(&fixture("cases/android/platform/documentation/PlatformProbe.java"))?;
	let output: Output = generate(root.path())?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	let code: String = String::from_utf8(output.stdout)?;
	assert!(code.contains("fn documented_value {"), "{code}");
	assert!(code.contains("sig = (documented_key: JString) -> JString"), "{code}");
	assert!(code.contains("#[deprecated]"), "{code}");
	assert!(
		code.contains("Keeps documentation mentioning @android.annotation.NonNull."),
		"{code}"
	);
	assert!(!code.contains("pub AndroidFont"), "{code}");
	Ok(())
}

#[test]
fn imported_android_annotations_match_fully_qualified_annotations() -> TestResult {
	let qualified: TempDir = sdk(&fixture("cases/android/platform/documentation/PlatformProbe.java"))?;
	let imported: TempDir = sdk(&fixture("cases/android/platform/imported/PlatformProbe.java"))?;
	let expected: Output = generate(qualified.path())?;
	let actual: Output = generate(imported.path())?;
	assert!(
		expected.status.success(),
		"{}",
		String::from_utf8_lossy(&expected.stderr)
	);
	assert!(actual.status.success(), "{}", String::from_utf8_lossy(&actual.stderr));
	assert_eq!(String::from_utf8(actual.stdout)?, String::from_utf8(expected.stdout)?);
	Ok(())
}

#[test]
fn retains_imports_used_by_api_types_and_other_annotations() -> TestResult {
	for (case, package) in [
		("imported_api_type", "android.annotation"),
		("other_annotation", "example.annotations"),
	] {
		let root: TempDir = sdk(&fixture(&format!("cases/android/platform/{case}/PlatformProbe.java")))?;
		let output: Output = generate(root.path())?;
		assert!(!output.status.success(), "{case} unexpectedly compiled");
		assert!(output.stdout.is_empty(), "partial bindings were written to stdout");
		let error: String = String::from_utf8(output.stderr)?;
		assert!(error.contains(&format!("package {package} does not exist")), "{error}");
	}
	Ok(())
}

#[test]
fn does_not_resolve_missing_platform_types_from_the_host_jdk() -> TestResult {
	let root: TempDir = sdk(&fixture("java/android/platform/host_types/PlatformProbe.java"))?;
	let output: Output = generate(root.path())?;
	assert!(!output.status.success());
	assert!(output.stdout.is_empty(), "partial bindings were written to stdout");
	let error: String = String::from_utf8(output.stderr)?;
	assert!(error.contains("PlatformProbe.java"), "{error}");
	assert!(error.contains("ProcessHandle"), "{error}");
	assert!(error.contains("cannot find symbol"), "{error}");
	Ok(())
}

#[test]
fn malformed_sdk_sources_still_fail() -> TestResult {
	let root: TempDir = sdk(&fixture("cases/invalid/android/PlatformProbe.java"))?;
	let output: Output = generate(root.path())?;
	assert!(!output.status.success());
	assert!(output.stdout.is_empty(), "partial bindings were written to stdout");
	assert!(String::from_utf8(output.stderr)?.contains("PlatformProbe.java"));
	Ok(())
}

#[test]
fn ordinary_sources_still_validate_android_annotations() {
	let source: PathBuf = fixture("java/android/platform/ordinary/PlatformProbe.java");
	let error: abos_jbindgen::Error = abos_jbindgen::Builder::new()
		.input_sources(vec![source], Vec::new(), Vec::new())
		.generate()
		.expect_err("ordinary source parsing must not erase Android annotations");
	assert!(error.to_string().contains("android.annotation"), "{error}");
}
