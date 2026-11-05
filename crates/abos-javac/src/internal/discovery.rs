use crate::{Error, Result};
use std::{
	env, fs,
	path::{Path, PathBuf},
	process::{Command, Output},
};

pub fn find_javac() -> Result<PathBuf> {
	if let Some(home) = env::var_os("JAVA_HOME") {
		let path: PathBuf = PathBuf::from(home)
			.join("bin")
			.join(if cfg!(windows) { "javac.exe" } else { "javac" });
		if path.is_file() {
			return Ok(path);
		}
	}
	which::which("javac")
		.map_err(|error| Error::CompilerNotFound(format!("{error}; set JAVA_HOME or add javac to PATH")))
}

pub fn version(path: &Path) -> Result<u32> {
	let output: Output = Command::new(path)
		.args(["-version", "-J-XX:+PerfDisableSharedMem"])
		.output()?;
	if output.status.success() {
		for bytes in [&output.stdout, &output.stderr] {
			let text: std::borrow::Cow<'_, str> = String::from_utf8_lossy(bytes);
			if let Some(version) = parse_version(&text) {
				return Ok(version);
			}
		}
	}
	Err(Error::CompilerNotFound(format!(
		"cannot detect javac version: {}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	)))
}

pub fn parse_version(text: &str) -> Option<u32> {
	for line in text.lines() {
		let Some(version) = line.trim().strip_prefix("javac ") else {
			continue;
		};
		let modern: &str = version.strip_prefix("1.").unwrap_or(version);
		let major: &str = modern.split(['.', '_', '-', '+', ' ']).next()?;
		if let Ok(number) = major.parse() {
			return Some(number);
		}
	}
	None
}

pub fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
	if !dir.is_dir() {
		return Err(Error::InvalidDirectory(dir.display().to_string()));
	}
	for entry in fs::read_dir(dir)? {
		let entry: fs::DirEntry = entry?;
		let path: PathBuf = entry.path();
		// Do not follow directory symlinks: source trees can contain cycles.
		if entry.file_type()?.is_dir() {
			scan_directory(&path, files)?;
		} else if path.is_file() && path.extension().is_some_and(|ext| ext == "java") {
			files.push(path);
		}
	}
	Ok(())
}
