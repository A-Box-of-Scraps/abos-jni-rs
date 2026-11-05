use super::{config::Config, discovery};
use crate::{Error, Result};
use std::{
	env,
	io::Write,
	path::{Path, PathBuf},
	sync::Arc,
};
use tempfile::NamedTempFile;

#[derive(Debug, Clone)]
pub struct Prepared {
	pub config: Config,
	pub path: PathBuf,
	pub version: u32,
	pub sources: Vec<PathBuf>,
	pub output_dir: PathBuf,
	pub output_src_dir: PathBuf,
	// Shared by snapshots so commands remain usable while any compiler clone lives.
	pub argument_file: Arc<NamedTempFile>,
}

impl Prepared {
	pub fn new(config: &Config) -> Result<Self> {
		let path: PathBuf = discovery::find_javac()?;
		let version: u32 = discovery::version(&path)?;
		let mut sources: Vec<PathBuf> = config.files.clone();
		for dir in &config.source_dirs {
			discovery::scan_directory(dir, &mut sources)?;
		}
		sources.sort();
		sources.dedup();
		let output_dir: PathBuf = output_dir(config)?;
		let output_src_dir: PathBuf = if let Some(dir) = &config.output_src_dir {
			dir.clone()
		} else if config.output_dir.is_some() {
			output_dir.clone()
		} else {
			cargo_out_dir()?.join("javac-build/generated-sources")
		};
		let mut argument_file: NamedTempFile = NamedTempFile::new()?;
		for source in &sources {
			writeln!(argument_file, "{}", quote_source(source)?)?;
		}
		argument_file.flush()?;
		Ok(Self {
			config: config.clone(),
			path,
			version,
			sources,
			output_dir,
			output_src_dir,
			argument_file: Arc::new(argument_file),
		})
	}
}

fn cargo_out_dir() -> Result<PathBuf> {
	env::var_os("OUT_DIR").map(PathBuf::from).ok_or(Error::OutDirNotSet)
}

fn output_dir(config: &Config) -> Result<PathBuf> {
	let base: PathBuf = match &config.output_dir {
		Some(dir) => dir.clone(),
		None => cargo_out_dir()?.join("javac-build/classes"),
	};
	Ok(config
		.output_subdir
		.as_ref()
		.map_or_else(|| base.clone(), |subdir| base.join(subdir)))
}

pub fn quote_source(path: &Path) -> Result<String> {
	let absolute: PathBuf = std::path::absolute(path)?;
	let text: &str = absolute
		.to_str()
		.ok_or_else(|| Error::InvalidSourcePath(absolute.clone()))?;
	// javac argument files interpret whitespace, quotes, backslashes, and comments.
	let escaped: String = text
		.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
		.replace('\r', "\\r")
		.replace('\t', "\\t");
	Ok(format!("\"{escaped}\""))
}
