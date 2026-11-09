//! Read actual core class files from the test JDK's runtime image.
use std::{
	fs,
	path::{Path, PathBuf},
	process::{Command, Output},
};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn entries(output: &Path) -> TestResult<Vec<(String, Vec<u8>)>> {
	let compiler: abos_javac::JavaCompiler = abos_javac::Build::new()
		.output_dir(output)
		.cargo_metadata(false)
		.try_get_compiler()?;
	let javac: PathBuf = compiler.path().canonicalize()?;
	let home: &Path = javac.parent().and_then(Path::parent).ok_or("missing JDK root")?;
	let classes: PathBuf = output.join("core-classes");
	let result: Output = Command::new(home.join("bin").join(format!("jimage{}", std::env::consts::EXE_SUFFIX)))
		.args([
			"extract",
			"--include",
			r"regex:/java.base/java/lang/(Object|Class|String|Throwable)\.class",
			"--dir",
		])
		.arg(&classes)
		.arg(home.join("lib/modules"))
		.output()?;
	assert!(
		result.status.success(),
		"{}{}",
		String::from_utf8_lossy(&result.stdout),
		String::from_utf8_lossy(&result.stderr)
	);
	let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
	for name in ["Object", "Class", "String", "Throwable"] {
		let path: String = format!("java/lang/{name}.class");
		let bytes: Vec<u8> = fs::read(classes.join("java.base").join(&path))?;
		entries.push((path, bytes));
	}
	Ok(entries)
}
