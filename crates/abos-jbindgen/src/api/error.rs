//! Error types for jbindgen

use std::{io, path::PathBuf};

/// Result type alias for jbindgen operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during binding generation
#[derive(Debug, thiserror::Error)]
pub enum Error {
	/// IO error reading class file
	#[error("IO error: {0}")]
	Io(#[from] io::Error),

	/// Error parsing class file with abos-cafebabe
	#[error("Failed to parse class file: {0}")]
	ParseClass(String),

	/// Error parsing Java sources
	#[error("Failed to parse Java sources: {0}")]
	Parse(String),

	/// Error reading JAR file
	#[error("Failed to read JAR file: {0}")]
	Zip(#[from] zip::result::ZipError),

	/// Android SDK related error
	#[error("Android SDK error: {0}")]
	AndroidSdk(String),

	/// A selected Android class could not be parsed from the platform archive.
	#[error("Failed to parse Android class {entry} in {archive}: {source}", archive = .archive.display())]
	AndroidClassParse {
		/// Platform JAR containing the class.
		archive: PathBuf,
		/// Archive entry name, including the `.class` suffix.
		entry: String,
		/// Underlying class-file or descriptor parsing error.
		#[source]
		source: Box<Self>,
	},

	/// Binding generation failed for a selected Android class.
	#[error("Failed to generate Android bindings for {class_name}: {source}")]
	AndroidClassGeneration {
		/// Dot-separated Java binary name.
		class_name: String,
		/// Underlying binding-generation error.
		#[source]
		source: Box<Self>,
	},

	/// Unsupported class feature
	#[error("Unsupported feature: {0}")]
	Unsupported(String),

	/// Code generation error
	#[error("Code generation error: {0}")]
	CodeGen(String),
}

impl From<abos_cafebabe::ParseError> for Error {
	/// Preserve the bytecode parser diagnostic.
	fn from(err: abos_cafebabe::ParseError) -> Self {
		Self::ParseClass(err.to_string())
	}
}
