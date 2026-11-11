use super::{config::Config, prepared::Prepared};
use crate::{Error, Result};
use std::{env, ffi::OsString, fs, path::PathBuf, process::Command};

pub fn command(compiler: &Prepared) -> Result<Command> {
	if compiler.sources.is_empty() {
		return Err(Error::NoSourceFiles);
	}
	let config: &Config = &compiler.config;
	let mut command = Command::new(&compiler.path);
	// Keep argument-file decoding and diagnostic parsing independent of the host locale.
	command.args([
		"-J-Dfile.encoding=UTF-8",
		"-J-Duser.language=en",
		"-J-Dsun.stdout.encoding=UTF-8",
		"-J-Dsun.stderr.encoding=UTF-8",
		"-J-Dstdout.encoding=UTF-8",
		"-J-Dstderr.encoding=UTF-8",
		"-J-XX:+PerfDisableSharedMem",
	]);
	command
		.arg("-d")
		.arg(&compiler.output_dir)
		.arg("-s")
		.arg(&compiler.output_src_dir);
	if !config.classpath.is_empty() {
		validate_classpath(&config.classpath, if cfg!(windows) { b';' } else { b':' })?;
		let classpath: OsString =
			env::join_paths(&config.classpath).map_err(|error| Error::InvalidClasspath(error.to_string()))?;
		command.arg("-classpath").arg(classpath);
	}
	if let Some(release) = &config.release {
		if compiler.version < 9 {
			return Err(Error::Unsupported(format!(
				"--release requires Java 9+, found Java {}",
				compiler.version
			)));
		}
		command.arg("--release").arg(release);
	} else {
		if let Some(source) = &config.source_version {
			command.arg("-source").arg(source);
		}
		if let Some(target) = &config.target_version {
			command.arg("-target").arg(target);
		}
	}
	if let Some(encoding) = &config.encoding {
		command.arg("-encoding").arg(encoding);
	}
	if config.debug {
		command.arg("-g");
	}
	if !config.warnings {
		command.arg("-nowarn");
	}
	if config.werror {
		command.arg("-Werror");
	}
	command.arg("-verbose").args(&config.extra_args);
	let mut argument: OsString = OsString::from("@");
	argument.push(compiler.argument_file.path());
	command.arg(argument);
	fs::create_dir_all(&compiler.output_dir)?;
	fs::create_dir_all(&compiler.output_src_dir)?;
	Ok(command)
}

pub fn validate_classpath(paths: &[PathBuf], separator: u8) -> Result<()> {
	// Windows PATH quoting does not make a Java classpath separator part of a filename.
	if paths
		.iter()
		.any(|path| path.as_os_str().as_encoded_bytes().contains(&separator))
	{
		return Err(Error::InvalidClasspath(format!(
			"entry contains separator '{}'",
			char::from(separator)
		)));
	}
	Ok(())
}
