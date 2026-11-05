use abos_jbindgen::Builder;
use std::path::PathBuf;
use tempfile::TempDir;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub fn fixture(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java").join(name)
}

pub fn source_builder(names: &[&str]) -> Builder {
	Builder::new().input_sources(names.iter().map(|name| fixture(name)).collect(), Vec::new(), Vec::new())
}

pub fn annotations() -> Result<TempDir, Box<dyn std::error::Error>> {
	let root: TempDir = tempfile::tempdir()?;
	let sources: Vec<PathBuf> = abos_jbindgen_annotations::write_sources(root.path().join("sources"))?;
	abos_javac::Build::new()
		.files(sources)
		.output_dir(root.path())
		.cargo_metadata(false)
		.try_compile()?;
	Ok(root)
}
