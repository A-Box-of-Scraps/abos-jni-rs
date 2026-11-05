use super::prepared::Prepared;
use crate::{Error, Result};
use std::{collections::HashSet, path::PathBuf, process::Output};

pub fn compile(compiler: &Prepared) -> Result<Vec<PathBuf>> {
	emit_metadata(compiler);
	let output: Output = super::command::command(compiler)?.output()?;
	let stdout: String = String::from_utf8(output.stdout)?;
	let stderr: String = String::from_utf8(output.stderr)?;
	if !output.status.success() {
		return Err(Error::CompilationFailed { stdout, stderr });
	}
	let classes: Vec<PathBuf> = parse_classes(&stderr);
	if classes.is_empty() {
		return Err(Error::NoClassFilesGenerated { stdout, stderr });
	}
	Ok(classes)
}

pub fn parse_classes(stderr: &str) -> Vec<PathBuf> {
	let mut seen: HashSet<&str> = HashSet::new();
	let mut classes: Vec<PathBuf> = Vec::new();
	for line in stderr.lines() {
		let Some(path) = line
			.trim()
			.strip_prefix("[wrote ")
			.and_then(|value| value.strip_suffix(']'))
		else {
			continue;
		};
		let path: &str = path
			.strip_prefix("RegularFileObject[")
			.and_then(|value| value.strip_suffix(']'))
			.unwrap_or(path);
		if std::path::Path::new(path).extension().is_some_and(|ext| ext == "class") && seen.insert(path) {
			classes.push(PathBuf::from(path));
		}
	}
	classes
}

fn emit_metadata(compiler: &Prepared) {
	if !compiler.config.cargo_metadata {
		return;
	}
	println!("cargo:rerun-if-env-changed=JAVA_HOME");
	println!("cargo:rerun-if-env-changed=PATH");
	if compiler.config.emit_rerun_if_changed {
		let mut seen: HashSet<&PathBuf> = HashSet::new();
		for path in compiler
			.sources
			.iter()
			.chain(&compiler.config.source_dirs)
			.chain(&compiler.config.classpath)
		{
			if seen.insert(path) {
				println!("cargo:rerun-if-changed={}", path.display());
			}
		}
	}
}
