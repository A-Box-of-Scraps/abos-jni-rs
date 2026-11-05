use std::{ffi::OsString, path::PathBuf};

#[allow(
	clippy::struct_excessive_bools,
	reason = "These independent javac switches are not mutually exclusive states"
)]
#[derive(Debug, Clone)]
pub struct Config {
	pub files: Vec<PathBuf>,
	pub source_dirs: Vec<PathBuf>,
	pub classpath: Vec<PathBuf>,
	pub output_dir: Option<PathBuf>,
	pub output_src_dir: Option<PathBuf>,
	pub output_subdir: Option<PathBuf>,
	pub source_version: Option<String>,
	pub target_version: Option<String>,
	pub release: Option<String>,
	pub encoding: Option<String>,
	pub debug: bool,
	pub warnings: bool,
	pub werror: bool,
	pub emit_rerun_if_changed: bool,
	pub cargo_metadata: bool,
	pub extra_args: Vec<OsString>,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			files: Vec::new(),
			source_dirs: Vec::new(),
			classpath: Vec::new(),
			output_dir: None,
			output_src_dir: None,
			output_subdir: None,
			source_version: None,
			target_version: None,
			release: None,
			encoding: None,
			debug: false,
			warnings: true,
			werror: false,
			emit_rerun_if_changed: false,
			cargo_metadata: true,
			extra_args: Vec::new(),
		}
	}
}
