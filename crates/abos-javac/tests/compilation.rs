use abos_javac::{Build, Error, JavaCompiler};
use std::{ffi::OsString, fs, path::PathBuf, process::Command};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Fixture {
	root: TempDir,
	output: PathBuf,
}

impl Fixture {
	fn new() -> std::io::Result<Self> {
		let root: TempDir = tempfile::tempdir()?;
		let output: PathBuf = root.path().join("class output");
		Ok(Self { root, output })
	}

	fn source(&self, name: &str, content: &str) -> std::io::Result<PathBuf> {
		let path: PathBuf = self.root.path().join("source space").join(name);
		fs::create_dir_all(path.parent().expect("source parent"))?;
		fs::write(&path, content)?;
		Ok(path)
	}

	fn copy_java(&self, source: &str, name: &str) -> std::io::Result<PathBuf> {
		let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.join("tests/java")
			.join(source);
		self.source(name, &fs::read_to_string(source)?)
	}

	fn build(&self) -> Build {
		let mut build = Build::new();
		build.output_dir(&self.output).cargo_metadata(false);
		build
	}
}

fn arguments(command: &Command) -> Vec<OsString> {
	command.get_args().map(OsString::from).collect()
}

fn argument_file(command: &Command) -> PathBuf {
	let argument: OsString = command.get_args().last().expect("source argument").to_os_string();
	PathBuf::from(
		argument
			.to_str()
			.expect("UTF-8 temporary path")
			.strip_prefix('@')
			.expect("argument file"),
	)
}

#[test]
fn compiles_only_selected_sources_and_reports_nested_classes() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("nested/example/Foo.java", "Foo.java")?;
	fixture.copy_java("basic/Bar.java", "Bar.java")?;
	fs::create_dir_all(&fixture.output)?;
	fs::write(fixture.output.join("Unrelated.class"), b"stale")?;
	let classes: Vec<PathBuf> = fixture.build().file(source).try_compile()?;
	assert_eq!(classes.len(), 2);
	for name in ["Foo.class", "Foo$Inner.class"] {
		assert!(classes.contains(&fixture.output.join("example").join(name)));
	}
	for class in classes {
		assert_eq!(&fs::read(class)?[..4], &[0xca, 0xfe, 0xba, 0xbe]);
	}
	Ok(())
}

#[test]
fn scans_recursively_and_deduplicates_explicit_inputs() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let foo: PathBuf = fixture.copy_java("dependent/Foo.java", "Foo.java")?;
	fixture.copy_java("basic/Bar.java", "nested/Bar.java")?;
	fixture.source("ignored.txt", "not Java")?;
	let classes: Vec<PathBuf> = fixture
		.build()
		.files([&foo, &foo])
		.source_dir(foo.parent().expect("parent"))
		.try_compile()?;
	assert_eq!(classes.len(), 2);
	Ok(())
}

#[test]
fn compiles_unicode_names_with_spaces_in_paths() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("unicode/Test你好Привет.java", "Test你好Привет.java")?;
	let classes: Vec<PathBuf> = fixture.build().file(source).encoding("UTF-8").try_compile()?;
	assert_eq!(classes, [fixture.output.join("Test你好Привет.class")]);
	Ok(())
}

#[test]
fn release_overrides_source_and_target_and_sets_class_version() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("basic/Foo.java", "Foo.java")?;
	let compiler: JavaCompiler = fixture
		.build()
		.file(source)
		.source_version("invalid")
		.target_version("invalid")
		.release("11")
		.try_get_compiler()?;
	let command: Command = compiler.to_command()?;
	let args: Vec<OsString> = arguments(&command);
	assert!(args.contains(&OsString::from("--release")));
	assert!(!args.contains(&OsString::from("-source")));
	assert!(!args.contains(&OsString::from("-target")));
	let classes: Vec<PathBuf> = compiler.try_compile()?;
	assert_eq!(&fs::read(&classes[0])?[6..8], &[0, 55]);
	Ok(())
}

#[test]
fn command_contains_configured_flags() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let compiler: JavaCompiler = fixture
		.build()
		.file("Foo.java")
		.source_version("11")
		.target_version("11")
		.debug(true)
		.warnings(false)
		.werror(true)
		.arg("-parameters")
		.args(["-Xlint:all", "-deprecation"])
		.remove_arg("-deprecation")
		.try_get_compiler()?;
	let command: Command = compiler.to_command()?;
	assert_eq!(command.get_program(), compiler.path());
	let args: Vec<OsString> = arguments(&command);
	for flag in [
		"-source",
		"-target",
		"-g",
		"-nowarn",
		"-Werror",
		"-parameters",
		"-Xlint:all",
		"-verbose",
	] {
		assert!(args.contains(&OsString::from(flag)), "{flag}");
	}
	assert!(!args.contains(&OsString::from("-deprecation")));
	Ok(())
}

#[test]
fn output_defaults_do_not_depend_on_setter_order() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let final_output: PathBuf = fixture.output.join("final");
	let compiler: JavaCompiler = fixture
		.build()
		.file("Foo.java")
		.output_dir("unused")
		.output_subdir("nested")
		.output_dir(&final_output)
		.try_get_compiler()?;
	let command: Command = compiler.to_command()?;
	let args: Vec<OsString> = arguments(&command);
	for flag in ["-d", "-s"] {
		let index: usize = args.iter().position(|arg| arg == flag).expect("output flag");
		assert_eq!(args[index + 1], final_output.join("nested"));
	}
	Ok(())
}

#[test]
fn explicit_generated_source_directory_is_preserved() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let generated: PathBuf = fixture.root.path().join("generated");
	let compiler: JavaCompiler = fixture
		.build()
		.file("Foo.java")
		.output_src_dir(&generated)
		.output_subdir("nested")
		.try_get_compiler()?;
	let command: Command = compiler.to_command()?;
	let args: Vec<OsString> = arguments(&command);
	let index: usize = args.iter().position(|arg| arg == "-s").expect("source output flag");
	assert_eq!(args[index + 1], generated);
	Ok(())
}

#[test]
fn errors_retain_compiler_diagnostics() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.source("Broken.java", include_str!("cases/invalid/Broken.java"))?;
	let error: Error = fixture.build().file(source).try_compile().expect_err("invalid Java");
	let Error::CompilationFailed { stderr, .. } = error else {
		panic!("unexpected error: {error}");
	};
	assert!(stderr.contains("Broken.java"));
	Ok(())
}

#[test]
fn rejects_missing_sources_and_invalid_directories() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	assert!(matches!(fixture.build().try_compile(), Err(Error::NoSourceFiles)));
	assert!(matches!(
		fixture
			.build()
			.source_dir(fixture.root.path().join("missing"))
			.try_compile(),
		Err(Error::InvalidDirectory(_))
	));
	Ok(())
}

#[test]
fn argument_files_are_unique_shared_by_clones_and_cleaned_up() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("basic/Foo.java", "Foo.java")?;
	let compiler: JavaCompiler = fixture.build().file(&source).try_get_compiler()?;
	let other: JavaCompiler = fixture.build().file(source).try_get_compiler()?;
	let clone: JavaCompiler = compiler.clone();
	let mut command: Command = compiler.to_command()?;
	let path: PathBuf = argument_file(&command);
	assert_ne!(path, argument_file(&other.to_command()?));
	drop(compiler);
	assert!(path.is_file());
	assert!(command.output()?.status.success());
	drop(clone);
	assert!(!path.exists());
	Ok(())
}

#[test]
fn classpath_resolves_previously_compiled_dependencies() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let dependency: PathBuf = fixture.copy_java("classpath/Dependency.java", "Dependency.java")?;
	let _classes: Vec<PathBuf> = fixture.build().file(dependency).try_compile()?;
	let consumer: PathBuf = fixture.copy_java("classpath/Consumer.java", "Consumer.java")?;
	let classes: Vec<PathBuf> = fixture
		.build()
		.file(consumer)
		.classpath(&fixture.output)
		.output_subdir("consumer")
		.try_compile()?;
	assert_eq!(classes, [fixture.output.join("consumer/Consumer.class")]);
	Ok(())
}

#[test]
fn compiler_is_a_snapshot_of_the_builder() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let foo: PathBuf = fixture.copy_java("basic/Foo.java", "Foo.java")?;
	let bar: PathBuf = fixture.copy_java("basic/Bar.java", "Bar.java")?;
	let mut build: Build = fixture.build();
	build.file(foo);
	let compiler: JavaCompiler = build.try_get_compiler()?;
	build.file(bar).arg("invalid-option");
	assert_eq!(compiler.compile(), [fixture.output.join("Foo.class")]);
	Ok(())
}

#[test]
fn compiles_utf16_sources() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.source("Encoded.java", "")?;
	let mut bytes: Vec<u8> = vec![0xff, 0xfe];
	for unit in include_str!("java/encoding/Encoded.java").encode_utf16() {
		bytes.extend(unit.to_le_bytes());
	}
	fs::write(&source, bytes)?;
	assert_eq!(
		fixture.build().file(source).encoding("UTF-16").compile(),
		[fixture.output.join("Encoded.class")]
	);
	Ok(())
}

#[cfg(unix)]
#[test]
fn preserves_non_utf8_extra_arguments() -> TestResult {
	use std::os::unix::ffi::OsStringExt;
	let fixture: Fixture = Fixture::new()?;
	let argument: OsString = OsString::from_vec(vec![0xff]);
	let compiler: JavaCompiler = fixture.build().file("Foo.java").args([&argument]).try_get_compiler()?;
	assert!(arguments(&compiler.to_command()?).contains(&argument));
	Ok(())
}

#[cfg(unix)]
#[test]
fn ignores_directory_symlink_cycles() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("basic/Foo.java", "Foo.java")?;
	let directory: &std::path::Path = source.parent().expect("source directory");
	std::os::unix::fs::symlink(directory, directory.join("cycle"))?;
	assert_eq!(
		fixture.build().source_dir(directory).compile(),
		[fixture.output.join("Foo.class")]
	);
	Ok(())
}

#[test]
fn rejects_unrepresentable_classpath_entries() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let compiler: JavaCompiler = fixture
		.build()
		.file("Foo.java")
		.classpath(if cfg!(windows) {
			"invalid;entry"
		} else {
			"invalid:entry"
		})
		.try_get_compiler()?;
	assert!(matches!(compiler.to_command(), Err(Error::InvalidClasspath(_))));
	Ok(())
}

#[test]
fn reports_successful_compilation_without_classes() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("empty/package-info.java", "package-info.java")?;
	assert!(matches!(
		fixture.build().file(source).try_compile(),
		Err(Error::NoClassFilesGenerated { .. })
	));
	Ok(())
}

#[cfg(unix)]
#[test]
fn compiles_paths_containing_argument_file_metacharacters() -> TestResult {
	let fixture: Fixture = Fixture::new()?;
	let source: PathBuf = fixture.copy_java("basic/Foo.java", "quotes\"and'apostrophes/#hash\\backslash/Foo.java")?;
	assert_eq!(
		fixture.build().file(source).compile(),
		[fixture.output.join("Foo.class")]
	);
	Ok(())
}
