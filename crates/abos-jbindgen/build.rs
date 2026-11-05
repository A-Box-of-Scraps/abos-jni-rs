//! Compile and embed the Java compiler-API parser.
use std::{env, fs, io::Write, path::PathBuf};
use zip::{ZipWriter, write::SimpleFileOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let output: PathBuf = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
	let classes: PathBuf = output.join("parser-classes");
	let files: Vec<PathBuf> = abos_javac::Build::new()
		.source_dir("java")
		.output_dir(&classes)
		.release("11")
		.encoding("UTF-8")
		.emit_rerun_if_changed(true)
		.try_compile()?;
	let jar: PathBuf = output.join("parser.jar");
	let mut archive: zip::ZipWriter<std::fs::File> = ZipWriter::new(fs::File::create(&jar)?);
	// Only embed files produced by this compilation, never stale build output.
	for file in files {
		let name: String = file.strip_prefix(&classes)?.to_string_lossy().replace('\\', "/");
		archive.start_file(name, SimpleFileOptions::default())?;
		archive.write_all(&fs::read(file)?)?;
	}
	archive.finish()?;
	println!("cargo:rustc-env=JBINDGEN_PARSER_JAR={}", jar.display());
	Ok(())
}
