//! Immutable Java fixtures shared by test binaries and their isolated JVM subprocesses.
use std::{
	collections::hash_map::DefaultHasher,
	fs::{self, File},
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

#[allow(dead_code, reason = "Only tests that load Java fixtures use this helper.")]
pub fn java_fixtures() -> PathBuf {
	// Trybuild compiles this module without CARGO_TARGET_TMPDIR but never calls it.
	#[allow(
		clippy::option_env_unwrap,
		reason = "Trybuild compiles but never executes this helper."
	)]
	let target: &str = option_env!("CARGO_TARGET_TMPDIR").expect("Cargo sets the integration test output directory");
	compile_fixtures(
		&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java"),
		&PathBuf::from(target).join("jni-java-fixtures"),
	)
	.expect("compile shared Java fixtures")
}

pub fn compile_fixtures(sources: &Path, cache: &Path) -> FixtureResult<PathBuf> {
	fs::create_dir_all(cache)?;
	let staging: tempfile::TempDir = tempfile::tempdir_in(cache)?;
	let mut files: Vec<PathBuf> = Vec::new();
	collect_sources(sources, &mut files)?;
	files.sort();

	let mut fingerprint: DefaultHasher = DefaultHasher::new();
	// Changes to compiler options or cache layout also invalidate the compiled fixtures.
	include_bytes!("fixtures.rs").hash(&mut fingerprint);
	let input: PathBuf = staging.path().join("sources");
	for file in files {
		let relative: &Path = file.strip_prefix(sources)?;
		let bytes: Vec<u8> = fs::read(&file)?;
		relative.hash(&mut fingerprint);
		bytes.hash(&mut fingerprint);
		let snapshot: PathBuf = input.join(relative);
		fs::create_dir_all(snapshot.parent().expect("source has a parent"))?;
		fs::write(snapshot, bytes)?;
	}

	let classes: PathBuf = staging.path().join("classes");
	let compiler: abos_javac::JavaCompiler = abos_javac::Build::new()
		.source_dir(&input)
		.output_dir(&classes)
		.encoding("UTF-8")
		.cargo_metadata(false)
		.try_get_compiler()?;
	let executable: PathBuf = compiler.path().canonicalize()?;
	let metadata: fs::Metadata = fs::metadata(&executable)?;
	executable.hash(&mut fingerprint);
	metadata.len().hash(&mut fingerprint);
	metadata.modified()?.hash(&mut fingerprint);
	// These options can affect javac even when its executable has not changed.
	for variable in [
		"JAVA_HOME",
		"JAVA_TOOL_OPTIONS",
		"JDK_JAVA_OPTIONS",
		"JDK_JAVAC_OPTIONS",
		"_JAVA_OPTIONS",
		"CLASSPATH",
	] {
		std::env::var_os(variable).hash(&mut fingerprint);
	}

	let key: String = format!("{:016x}", fingerprint.finish());
	let output: PathBuf = cache.join(&key);
	let lock: File = File::options()
		.create(true)
		.read(true)
		.write(true)
		.truncate(false)
		.open(cache.join(format!("{key}.lock")))?;
	// A process-local OnceLock cannot coordinate rusty-fork children or other test binaries.
	lock.lock()?;
	// Rust cache cleanup can leave directories after deleting their class files.
	if !output.join(".complete").is_file() {
		if output.is_dir() {
			fs::remove_dir_all(&output)?;
		}
		compiler.try_compile()?;
		fs::write(classes.join(".complete"), [])?;
		// Publish only complete output. Failed compilations leave no reusable cache entry.
		fs::rename(&classes, &output)?;
	}
	Ok(output)
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
	for entry in fs::read_dir(directory)? {
		let entry: fs::DirEntry = entry?;
		let kind: fs::FileType = entry.file_type()?;
		let path: PathBuf = entry.path();
		if kind.is_dir() {
			collect_sources(&path, files)?;
		} else if kind.is_file() && path.extension().is_some_and(|extension| extension == "java") {
			files.push(path);
		}
	}
	Ok(())
}
