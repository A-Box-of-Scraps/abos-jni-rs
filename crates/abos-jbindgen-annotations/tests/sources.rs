//! Verify that exported annotations are self-contained and compile with our javac wrapper.
use std::{fs, path::PathBuf};
use tempfile::TempDir;

#[test]
fn exports_and_compiles_annotations() -> Result<(), Box<dyn std::error::Error>> {
	let root: TempDir = tempfile::tempdir()?;
	let files: Vec<PathBuf> = abos_jbindgen_annotations::write_sources(root.path())?;
	assert_eq!(files.len(), abos_jbindgen_annotations::ANNOTATION_FILES.len());
	for (file, (_, content)) in files.iter().zip(abos_jbindgen_annotations::ANNOTATION_FILES) {
		assert_eq!(fs::read_to_string(file)?, *content);
	}
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.files(files)
		.output_dir(root.path().join("classes"))
		.cargo_metadata(false)
		.try_compile()?;
	for name in ["RustName", "RustPrimitive", "RustSkip"] {
		assert!(classes.iter().any(|path| path.ends_with(format!("{name}.class"))));
	}
	Ok(())
}
