//! Source and bytecode inputs share the same class-selection contract.
#[path = "support/android_platform.rs"]
mod android_platform;

use abos_jbindgen::{Bindings, Builder, ModuleEvent};
use std::{
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Output},
	thread,
};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LOWERCASE: [&str; 3] = ["com.UPPER.lower", "com.UPPER.lower$inner", "com.UPPER.lower$inner$deep"];
const OUTER: [&str; 5] = [
	"com.example.PatternOuter",
	"com.example.PatternOuter$Inner",
	"com.example.PatternOuter$Inner$Deep",
	"com.example.PatternOuter$InnerSibling",
	"com.example.PatternOuter$Sibling",
];

fn class_names(bindings: &Bindings) -> Vec<String> {
	let mut names: Vec<String> = Vec::new();
	bindings.walk(&mut |module, event| {
		if event == ModuleEvent::BeginModule {
			names.extend(
				module
					.bindings
					.iter()
					.map(|binding| format!("{}.{}", binding.package, binding.java_type_name)),
			);
		}
	});
	names.sort();
	names
}

struct Fixtures {
	root: TempDir,
	sources: Vec<PathBuf>,
	jar: PathBuf,
	source_jar: PathBuf,
}

fn fixtures() -> TestResult<Fixtures> {
	let root: TempDir = tempfile::tempdir()?;
	let fixtures: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java");
	let sources: Vec<PathBuf> = vec![fixtures.join("PatternOuter.java"), fixtures.join("patterns/lower.java")];
	let classes: PathBuf = root.path().join("classes");
	let files: Vec<PathBuf> = abos_javac::Build::new()
		.files(&sources)
		.output_dir(&classes)
		.release("8")
		.cargo_metadata(false)
		.try_compile()?;
	let jar: PathBuf = root.path().join("patterns.jar");
	let platform: PathBuf = root.path().join("platforms/android-35");
	fs::create_dir_all(&platform)?;
	let mut android: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(platform.join("android.jar"))?);
	let mut archive: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(&jar)?);
	for file in files {
		let name: String = file.strip_prefix(&classes)?.to_string_lossy().replace('\\', "/");
		let bytes: Vec<u8> = fs::read(file)?;
		for writer in [&mut archive, &mut android] {
			writer.start_file(&name, SimpleFileOptions::default())?;
			writer.write_all(&bytes)?;
		}
	}
	archive.finish()?;
	android_platform::add_core_classes(&mut android)?;
	android.finish()?;
	let source_jar: PathBuf = platform.join("android-stubs-src.jar");
	let mut archive: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(&source_jar)?);
	for source in &sources {
		let name: String = source.strip_prefix(&fixtures)?.to_string_lossy().replace('\\', "/");
		archive.start_file(name, SimpleFileOptions::default())?;
		archive.write_all(&fs::read(source)?)?;
	}
	archive.finish()?;
	Ok(Fixtures {
		root,
		sources,
		jar,
		source_jar,
	})
}

fn android_class_names(sdk: &Path, patterns: &[&str], all: &[&str]) -> TestResult<Vec<String>> {
	// Isolate SDK discovery in a subprocess instead of mutating the test process's environment.
	let mut command: Command = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"));
	command
		.args(["android", "--api-level", "35"])
		.env("ANDROID_HOME", sdk)
		.env_remove("ANDROID_SDK_ROOT");
	for pattern in patterns {
		command.args(["--pattern", pattern]);
	}
	let output: Output = command.output()?;
	assert!(
		output.status.success(),
		"{patterns:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let code: String = String::from_utf8(output.stdout)?;
	let names: Vec<String> = all
		.iter()
		.filter(|name| code.contains(&format!("=> \"{name}\"")))
		.map(|name| (*name).to_owned())
		.collect();
	assert_eq!(
		code.matches("bind_java_type! {").count(),
		names.len(),
		"unexpected Android classes: {code}"
	);
	Ok(names)
}

#[test]
fn all_input_modes_select_the_same_classes() -> TestResult {
	let fixtures: Fixtures = fixtures()?;

	let all: Vec<&str> = LOWERCASE.iter().chain(&OUTER).copied().collect();
	let union: [&str; 3] = [OUTER[1], OUTER[2], OUTER[4]];
	let cases: &[(&[&str], &[&str])] = &[
		(&[], &all),
		(&["*"], &all),
		(&["**"], &all),
		(&["com.example.PatternOuter"], &OUTER),
		(&["com.example.*"], &OUTER),
		(&["com.*"], &all),
		(&["com.example.PatternOuter.Inner"], &OUTER[1..3]),
		(&["com.example.PatternOuter$Inner"], &OUTER[1..3]),
		(&["com.example.PatternOuter.Inner.Deep"], &OUTER[2..3]),
		(&["com.example.PatternOuter$Inner.Deep"], &OUTER[2..3]),
		(&["com.example.PatternOuter$Inner$Deep"], &OUTER[2..3]),
		(&["com.example.PatternOuter$*"], &OUTER[1..]),
		(&["com.example.PatternOuter.*"], &OUTER[1..]),
		(&["com.example.PatternOuter$Inner$*"], &OUTER[2..3]),
		(&["com.example.PatternOuter.Inner*"], &OUTER[1..4]),
		(&["*Inner"], &OUTER[1..2]),
		(&["*Inner*"], &OUTER[1..4]),
		(&["*Outer*Deep"], &OUTER[2..3]),
		(&["com.**.PatternOuter**Deep"], &OUTER[2..3]),
		(&["com/example/PatternOuter$Inner"], &OUTER[1..3]),
		(&["com.UPPER.lower"], &LOWERCASE),
		(&["com.UPPER.lower.inner"], &LOWERCASE[1..]),
		(&["com.UPPER.lower$inner"], &LOWERCASE[1..]),
		(&["com.UPPER.lower$*"], &LOWERCASE[1..]),
		(&["com.UPPER.lower.inner.deep"], &LOWERCASE[2..]),
		(
			&["com.example.PatternOuter.Inner", "com.example.PatternOuter$Sibling"],
			&union,
		),
		(
			&[" com.example.PatternOuter.Inner, com.example.PatternOuter$Sibling "],
			&union,
		),
		(&["com.example.PatternOuter", "com.example.PatternOuter.Inner"], &OUTER),
		(&["\tcom.example.PatternOuter.Inner\r\n"], &OUTER[1..3]),
		(&["\u{00a0}com.example.PatternOuter.Inner\u{2003}"], &OUTER[1..3]),
		(&["\u{001c}com.example.PatternOuter.Inner"], &[]),
		(&["com.example"], &[]),
		(&["com.example.Pattern"], &[]),
		(&["PatternOuter"], &[]),
		(&["com.example.patternouter"], &[]),
		(&["com.upper.lower"], &[]),
		(&["no.match.*"], &[]),
		(&["com.example.PatternOuter?"], &[]),
		(&["com.example.PatternOuter[.*"], &[]),
		(&["com.example.PatternOuter|*"], &[]),
		(&["com.example.PatternOuter(*"], &[]),
		(&["com.example.PatternOuter+*"], &[]),
		(&["", " , "], &[]),
	];
	// Bound concurrent Java processes while sharing the immutable fixtures.
	let workers: usize = thread::available_parallelism().map_or(1, |count| count.get().min(4));
	thread::scope(|scope| {
		for chunk in cases.chunks(cases.len().div_ceil(workers)) {
			let fixtures: &Fixtures = &fixtures;
			let all: &[&str] = &all;
			scope.spawn(move || check_cases(fixtures, chunk, all));
		}
	});
	Ok(())
}

fn check_cases(fixtures: &Fixtures, cases: &[(&[&str], &[&str])], all: &[&str]) {
	for (patterns, expected) in cases {
		check_case(fixtures, patterns, expected, all).unwrap_or_else(|error| panic!("patterns {patterns:?}: {error}"));
	}
}

fn check_case(fixtures: &Fixtures, patterns: &[&str], expected: &[&str], all: &[&str]) -> TestResult {
	for (kind, builder) in [
		(
			"source",
			Builder::new().input_sources(fixtures.sources.clone(), Vec::new(), Vec::new()),
		),
		(
			"source jar",
			Builder::new().input_sources(vec![fixtures.source_jar.clone()], Vec::new(), Vec::new()),
		),
		("jar", Builder::new().input_jar(&fixtures.jar)),
	] {
		let bindings: Bindings = builder.patterns(patterns.iter().copied()).generate()?;
		assert_eq!(class_names(&bindings), expected, "{kind}: {patterns:?}");
	}
	assert_eq!(
		android_class_names(fixtures.root.path(), patterns, all)?,
		expected,
		"android: {patterns:?}"
	);
	Ok(())
}
