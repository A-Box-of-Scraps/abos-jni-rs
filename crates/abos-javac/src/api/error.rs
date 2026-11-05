use std::{path::PathBuf, string::FromUtf8Error};

/// Result of a Java compilation operation.
pub type Result<T> = std::result::Result<T, Error>;

/// Compiler discovery, configuration, or compilation failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
	/// Filesystem or process operation failed.
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	/// Compiler output was not UTF-8.
	#[error("UTF-8 conversion error: {0}")]
	Utf8(#[from] FromUtf8Error),
	/// Compiler discovery or version detection failed.
	#[error("javac compiler not found: {0}")]
	CompilerNotFound(String),
	/// No Java sources were selected.
	#[error("No source files specified for compilation")]
	NoSourceFiles,
	/// The compiler returned a failing exit status.
	#[error("javac compilation failed:\nstdout: {stdout}\nstderr: {stderr}")]
	CompilationFailed {
		/// Captured standard output.
		stdout: String,
		/// Captured diagnostics.
		stderr: String,
	},
	/// Successful compilation did not report any class files.
	#[error("javac reported no generated class files:\nstdout: {stdout}\nstderr: {stderr}")]
	NoClassFilesGenerated {
		/// Captured standard output.
		stdout: String,
		/// Captured diagnostics.
		stderr: String,
	},
	/// A classpath entry contains the platform's path-list separator.
	#[error("invalid classpath: {0}")]
	InvalidClasspath(String),
	/// A source directory is not a directory.
	#[error("not a directory: {0}")]
	InvalidDirectory(String),
	/// Neither an explicit output directory nor Cargo's `OUT_DIR` was available.
	#[error("OUT_DIR environment variable not set and no output directory specified")]
	OutDirNotSet,
	/// A requested option is unsupported by the selected JDK.
	#[error("unsupported: {0}")]
	Unsupported(String),
	/// A source path cannot be represented in a UTF-8 argument file.
	#[error("source path is not UTF-8: {0}")]
	InvalidSourcePath(PathBuf),
}
