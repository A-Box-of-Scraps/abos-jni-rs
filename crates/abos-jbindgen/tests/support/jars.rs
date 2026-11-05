//! Real class-file fixtures shared by archive tests and the generated-code build.
use std::{
	fs,
	io::Write,
	path::{Path, PathBuf},
};
use zip::{ZipWriter, write::SimpleFileOptions};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub struct Fixtures {
	pub modular: PathBuf,
	pub multi_release: PathBuf,
	pub reversed: PathBuf,
}

fn compile(sources: &Path, output: &Path, release: &str) -> TestResult<Vec<(String, Vec<u8>)>> {
	let files: Vec<PathBuf> = abos_javac::Build::new()
		.source_dir(sources)
		.output_dir(output)
		.release(release)
		.encoding("UTF-8")
		.cargo_metadata(false)
		.try_compile()?;
	files
		.into_iter()
		.map(|path| {
			let name: String = path
				.strip_prefix(output)?
				.to_str()
				.ok_or("non-UTF-8 class path")?
				.replace('\\', "/");
			Ok((name, fs::read(path)?))
		})
		.collect()
}

pub fn write_jar(path: &Path, entries: &[(String, Vec<u8>)]) -> TestResult {
	let mut archive: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(path)?);
	for (name, bytes) in entries {
		archive.start_file(name, SimpleFileOptions::default())?;
		archive.write_all(bytes)?;
	}
	archive.finish()?;
	Ok(())
}

pub fn create(sources: &Path, output: &Path) -> TestResult<Fixtures> {
	fs::create_dir_all(output)?;
	let mut entries: Vec<(String, Vec<u8>)> = compile(&sources.join("modular"), &output.join("modular"), "9")?;
	let modular: PathBuf = output.join("modular.jar");
	write_jar(&modular, &entries)?;
	entries.extend(compile(&sources.join("base"), &output.join("base"), "9")?);
	entries.extend(
		compile(&sources.join("v17"), &output.join("v17"), "17")?
			.into_iter()
			.map(|(name, bytes)| (format!("META-INF/versions/17/{name}"), bytes)),
	);
	entries.push((
		"META-INF/MANIFEST.MF".to_owned(),
		b"Manifest-Version: 1.0\r\nMulti-Release: true\r\n\r\n".to_vec(),
	));
	let multi_release: PathBuf = output.join("multi-release.jar");
	write_jar(&multi_release, &entries)?;
	entries.reverse();
	let reversed: PathBuf = output.join("reversed.jar");
	write_jar(&reversed, &entries)?;
	Ok(Fixtures {
		modular,
		multi_release,
		reversed,
	})
}
