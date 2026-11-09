//! Supply core APIs in synthetic SDKs instead of relying on the parser's host JDK.
use std::{fs, io, path::PathBuf};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub fn add_core_classes(archive: &mut ZipWriter<fs::File>) -> Result<(), Box<dyn std::error::Error>> {
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let compiler: abos_javac::JavaCompiler = abos_javac::Build::new()
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_get_compiler()?;
	let javac: PathBuf = compiler.path().canonicalize()?;
	let home: &std::path::Path = javac
		.parent()
		.and_then(std::path::Path::parent)
		.ok_or("missing JDK root")?;
	// ct.sym contains class-file signatures, grouped by supported Java releases.
	// Embed Java 8's core APIs in android.jar, just as a real SDK supplies its own.
	let mut signatures: ZipArchive<fs::File> = ZipArchive::new(fs::File::open(home.join("lib/ct.sym"))?)?;
	let mut count: usize = 0;
	for index in 0..signatures.len() {
		let mut entry: zip::read::ZipFile<'_, fs::File> = signatures.by_index(index)?;
		let name: String = entry.name()?.into_owned();
		let mut parts: std::str::SplitN<'_, char> = name.splitn(3, '/');
		let releases: &str = parts.next().unwrap_or_default();
		let module: &str = parts.next().unwrap_or_default();
		let path: &str = parts.next().unwrap_or_default();
		if releases.contains('8')
			&& module == "java.base"
			&& let Some(class) = path.strip_suffix(".sig")
		{
			archive.start_file(format!("{class}.class"), SimpleFileOptions::default())?;
			io::copy(&mut entry, archive)?;
			count += 1;
		}
	}
	assert!(count > 0, "JDK ct.sym must supply Java 8 core classes");
	Ok(())
}
