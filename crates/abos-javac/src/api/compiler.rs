use crate::{
	Result,
	internal::{command, config::Config, output, prepared::Prepared},
};
use std::{
	path::{Path, PathBuf},
	process::Command,
};

/// A resolved compiler and immutable snapshot of its inputs and options.
///
/// Owns a temporary source argument file, shared by clones and removed on drop.
#[derive(Debug, Clone)]
pub struct JavaCompiler {
	prepared: Prepared,
}

impl JavaCompiler {
	/// Resolve a build configuration and prepare its source argument file.
	///
	/// # Errors
	/// Returns discovery, source scanning, output resolution, or argument-file errors.
	pub(crate) fn new(config: &Config) -> Result<Self> {
		Ok(Self {
			prepared: Prepared::new(config)?,
		})
	}

	/// Return the resolved `javac` executable path.
	#[must_use]
	pub fn path(&self) -> &Path {
		&self.prepared.path
	}

	/// Create a command and its output directories without running the compiler.
	///
	/// Keep this compiler (or a clone) alive until the command has finished: it owns
	/// the source argument file. Sources are absolute; output and classpath paths
	/// retain the caller's spelling, so take care when changing the working directory.
	///
	/// Running the command directly does not emit Cargo metadata or collect class-file
	/// paths. Handle its exit status and output yourself.
	///
	/// # Examples
	///
	/// ```no_run
	/// # fn main() -> abos_javac::Result<()> {
	/// let compiler = abos_javac::Build::new()
	///     .file("java/Example.java")
	///     .output_dir("target/java-classes")
	///     .try_get_compiler()?;
	///
	/// let mut child = compiler.to_command()?.spawn()?;
	/// let status = child.wait()?;
	///
	/// // Keep the source argument file alive until the child process has finished.
	/// drop(compiler);
	/// assert!(status.success());
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	/// Returns an error for missing sources, invalid options, or filesystem failures.
	pub fn to_command(&self) -> Result<Command> {
		command::command(&self.prepared)
	}

	/// Compile and return only class files reported by this invocation.
	///
	/// # Panics
	/// Panics on configuration, I/O, compilation, or diagnostic decoding failure.
	#[must_use]
	pub fn compile(&self) -> Vec<PathBuf> {
		self.try_compile().unwrap_or_else(|error| panic!("{error}"))
	}

	/// Compile and return only class files reported by this invocation.
	///
	/// # Errors
	/// Returns configuration, I/O, compilation, or diagnostic decoding errors.
	/// Compiler failures include captured stdout and stderr.
	pub fn try_compile(&self) -> Result<Vec<PathBuf>> {
		output::compile(&self.prepared)
	}
}
