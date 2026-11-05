//! Android SDK discovery tests using a temporary SDK layout, without Android tooling.
use abos_jbindgen::{AndroidSdk, Error};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

#[test]
fn validates_sdk_files_and_optional_annotation_versions() -> Result<(), Box<dyn std::error::Error>> {
	let root: TempDir = tempfile::tempdir()?;
	assert!(AndroidSdk::from_path(root.path().join("missing"), 35).is_err());
	let sdk: AndroidSdk = AndroidSdk::from_path(root.path().to_owned(), 35)?;
	assert!(matches!(sdk.get_android_jar(), Err(Error::AndroidSdk(_))));
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	fs::write(platform.join("android.jar"), [])?;
	fs::write(platform.join("android-stubs-src.jar"), [])?;
	assert_eq!(sdk.get_stubs_src_jar()?, platform.join("android-stubs-src.jar"));
	for version in ["1.9.0", "1.10.0"] {
		let path: PathBuf = root
			.path()
			.join("extras/m2repository/androidx/annotation/annotation")
			.join(version);
		fs::create_dir_all(&path)?;
		fs::write(path.join("annotation.jar"), [])?;
		fs::write(path.join("annotation-sources.jar"), [])?;
	}
	let classpath: Vec<PathBuf> = sdk.get_classpath()?;
	assert_eq!(classpath.len(), 2);
	assert!(classpath[1].ends_with("1.10.0/annotation.jar"));
	Ok(())
}

#[test]
fn generates_and_filters_android_bindings_from_local_sdk_archives() -> Result<(), Box<dyn std::error::Error>> {
	use std::{
		io::Write,
		process::{Command, Output},
	};
	use zip::{ZipWriter, write::SimpleFileOptions};
	let root: TempDir = tempfile::tempdir()?;
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.file(&source)
		.output_dir(root.path().join("classes"))
		.cargo_metadata(false)
		.try_compile()?;
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	for (archive, entry, bytes) in [
		("android.jar", "com/example/SimpleClass.class", fs::read(&classes[0])?),
		(
			"android-stubs-src.jar",
			"com/example/SimpleClass.java",
			fs::read(source)?,
		),
	] {
		let mut writer: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join(archive))?);
		writer.start_file(entry, SimpleFileOptions::default())?;
		writer.write_all(&bytes)?;
		writer.finish()?;
	}
	let flags: PathBuf = root.path().join("hidden-api.csv");
	fs::write(
		&flags,
		"Lcom/example/SimpleClass;->add(II)I,public-api\nLcom/example/SimpleClass;->getValue()I,blocked\n",
	)?;
	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.args([
			"android",
			"--api-level",
			"35",
			"--pattern",
			"com.example.*",
			"--hiddenapi-flags",
		])
		.arg(flags)
		.env("ANDROID_HOME", root.path())
		.env_remove("ANDROID_SDK_ROOT")
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	let code: String = String::from_utf8(output.stdout)?;
	assert!(code.contains("fn add("));
	assert!(!code.contains("fn get_value("));
	assert!(!code.contains("fn new("));
	Ok(())
}
