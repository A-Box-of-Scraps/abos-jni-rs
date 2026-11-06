// Copyright 2019 astonbitecode
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Modified for abos-jni: JVM-only discovery, native paths, and cycle-safe traversal.

use std::{
	collections::HashSet,
	env,
	ffi::OsString,
	fs, io,
	path::{Path, PathBuf},
	process::{Command, Output},
};

const JVM_LIBRARY: &str = if cfg!(windows) {
	"jvm.dll"
} else if cfg!(target_os = "macos") {
	"libjvm.dylib"
} else {
	"libjvm.so"
};

pub fn locate_jvm() -> io::Result<PathBuf> {
	locate_with(env::var_os("JAVA_HOME"), discover_home)
}

fn locate_with(home: Option<OsString>, discover: impl FnOnce() -> io::Result<PathBuf>) -> io::Result<PathBuf> {
	let home: PathBuf = match home.filter(|value| !value.is_empty()) {
		Some(value) => value.into(),
		None => discover()?,
	};
	find_library(&home)
}

fn discover_home() -> io::Result<PathBuf> {
	let program: &str = if cfg!(target_os = "macos") {
		"/usr/libexec/java_home"
	} else if cfg!(windows) {
		"where"
	} else {
		"which"
	};
	let mut command: Command = Command::new(program);
	if !cfg!(target_os = "macos") {
		command.arg("java");
	}
	let output: Output = command
		.output()
		.map_err(|error| io::Error::new(error.kind(), format!("Could not run {program}: {error}")))?;
	if !output.status.success() {
		return Err(io::Error::new(
			io::ErrorKind::NotFound,
			format!(
				"{program} could not locate Java: {}",
				String::from_utf8_lossy(&output.stderr).trim()
			),
		));
	}
	home_from_output(&output.stdout, cfg!(target_os = "macos"))
}

fn home_from_output(stdout: &[u8], is_home: bool) -> io::Result<PathBuf> {
	let text: &str = std::str::from_utf8(stdout).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
	let first: &str = text.lines().next().unwrap_or_default().trim();
	if first.is_empty() {
		return Err(io::Error::new(
			io::ErrorKind::NotFound,
			"Java is not installed or not on PATH",
		));
	}
	let path: PathBuf = fs::canonicalize(first)?;
	if is_home {
		return Ok(path);
	}
	path.parent()
		.and_then(Path::parent)
		.map(Path::to_path_buf)
		.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::InvalidData,
				format!("Cannot determine Java home from {}", path.display()),
			)
		})
}

fn find_library(home: &Path) -> io::Result<PathBuf> {
	// Search all layouts, including architecture-specific Java 8 JRE directories.
	// Canonical paths prevent symlink cycles without excluding linked installations.
	let mut pending: Vec<PathBuf> = vec![home.to_path_buf()];
	let mut visited: HashSet<PathBuf> = HashSet::new();
	while let Some(directory) = pending.pop() {
		let Ok(canonical) = fs::canonicalize(&directory) else {
			continue;
		};
		if !visited.insert(canonical) {
			continue;
		}
		let library: PathBuf = directory.join(JVM_LIBRARY);
		if library.is_file() {
			return Ok(library);
		}
		let Ok(entries) = fs::read_dir(&directory) else {
			continue;
		};
		let mut children: Vec<PathBuf> = entries
			.filter_map(std::result::Result::ok)
			.map(|entry| entry.path())
			.filter(|path| path.is_dir())
			.collect();
		// Reverse insertion gives deterministic ascending depth-first traversal.
		children.sort();
		pending.extend(children.into_iter().rev());
	}
	Err(io::Error::new(
		io::ErrorKind::NotFound,
		format!("Could not find {JVM_LIBRARY} in Java installation {}", home.display()),
	))
}

#[cfg(test)]
#[path = "../../tests/java_locator/mod.rs"]
mod tests;
