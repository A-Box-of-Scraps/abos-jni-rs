use crate::{JavaCompiler, Result, internal::config::Config};
use std::{
	ffi::OsStr,
	path::{Path, PathBuf},
};

/// Mutable configuration for a Java compilation.
#[derive(Debug, Clone, Default)]
pub struct Build {
	pub(crate) config: Config,
}

impl Build {
	/// Create a builder with warnings and Cargo metadata enabled.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a Java source file.
	pub fn file(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.files.push(path.as_ref().to_path_buf());
		self
	}

	/// Recursively include Java sources from a directory. Directory symlinks are not followed.
	pub fn source_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.source_dirs.push(path.as_ref().to_path_buf());
		self
	}

	/// Append a directory or JAR to the classpath.
	pub fn classpath(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.classpath.push(path.as_ref().to_path_buf());
		self
	}

	/// Add multiple Java source files.
	pub fn files<P: AsRef<Path>>(&mut self, paths: impl IntoIterator<Item = P>) -> &mut Self {
		self.config
			.files
			.extend(paths.into_iter().map(|path| path.as_ref().to_path_buf()));
		self
	}

	/// Set the class output directory; defaults to `$OUT_DIR/javac-build/classes`.
	pub fn output_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.output_dir = Some(path.as_ref().to_path_buf());
		self
	}

	/// Append a path to the class output directory.
	pub fn output_subdir(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.output_subdir = Some(path.as_ref().to_path_buf());
		self
	}

	/// Set generated-source output; defaults to the resolved explicit class output, or `$OUT_DIR/javac-build/generated-sources`.
	pub fn output_src_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
		self.config.output_src_dir = Some(path.as_ref().to_path_buf());
		self
	}

	/// Set `-source`; ignored when `release` is set.
	pub fn source_version(&mut self, value: impl Into<String>) -> &mut Self {
		self.config.source_version = Some(value.into());
		self
	}

	/// Set `-target`; ignored when `release` is set.
	pub fn target_version(&mut self, value: impl Into<String>) -> &mut Self {
		self.config.target_version = Some(value.into());
		self
	}

	/// Set `--release` (requires JDK 9+), overriding source and target versions.
	pub fn release(&mut self, value: impl Into<String>) -> &mut Self {
		self.config.release = Some(value.into());
		self
	}

	/// Set the Java source encoding (`-encoding`).
	pub fn encoding(&mut self, value: impl Into<String>) -> &mut Self {
		self.config.encoding = Some(value.into());
		self
	}

	/// Enable full debug information (`-g`); disabled by default.
	pub const fn debug(&mut self, enable: bool) -> &mut Self {
		self.config.debug = enable;
		self
	}

	/// Enable compiler warnings; enabled by default.
	pub const fn warnings(&mut self, enable: bool) -> &mut Self {
		self.config.warnings = enable;
		self
	}

	/// Treat warnings as errors; disabled by default.
	pub const fn werror(&mut self, enable: bool) -> &mut Self {
		self.config.werror = enable;
		self
	}

	/// Emit Cargo source-change directives; defaults to false and requires Cargo metadata.
	pub const fn emit_rerun_if_changed(&mut self, enable: bool) -> &mut Self {
		self.config.emit_rerun_if_changed = enable;
		self
	}

	/// Enable Cargo metadata output; enabled by default.
	pub const fn cargo_metadata(&mut self, enable: bool) -> &mut Self {
		self.config.cargo_metadata = enable;
		self
	}

	/// Append an argument without UTF-8 conversion.
	pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
		self.config.extra_args.push(arg.as_ref().to_os_string());
		self
	}

	/// Append arguments without UTF-8 conversion.
	pub fn args<S: AsRef<OsStr>>(&mut self, args: impl IntoIterator<Item = S>) -> &mut Self {
		self.config
			.extra_args
			.extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
		self
	}

	/// Remove every occurrence of an extra argument.
	pub fn remove_arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
		self.config.extra_args.retain(|value| value != arg.as_ref());
		self
	}

	/// Compile sources and return only class files reported by this invocation.
	///
	/// # Panics
	/// Panics on configuration, I/O, or compilation failure.
	#[must_use]
	pub fn compile(&self) -> Vec<PathBuf> {
		self.try_compile().unwrap_or_else(|error| panic!("{error}"))
	}

	/// Compile sources and return only class files reported by this invocation.
	///
	/// # Errors
	/// Returns discovery, configuration, I/O, or compiler errors with diagnostics.
	pub fn try_compile(&self) -> Result<Vec<PathBuf>> {
		self.try_get_compiler()?.try_compile()
	}

	/// Resolve the compiler and snapshot this configuration.
	///
	/// # Panics
	/// Panics if discovery, source scanning, or output resolution fails.
	#[must_use]
	pub fn get_compiler(&self) -> JavaCompiler {
		self.try_get_compiler().unwrap_or_else(|error| panic!("{error}"))
	}

	/// Resolve the compiler and snapshot this configuration.
	///
	/// # Errors
	/// Returns discovery, source scanning, output resolution, or argument-file errors.
	pub fn try_get_compiler(&self) -> Result<JavaCompiler> {
		JavaCompiler::new(&self.config)
	}
}
