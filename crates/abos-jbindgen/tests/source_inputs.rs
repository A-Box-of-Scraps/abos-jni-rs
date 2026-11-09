//! Source input validation remains distinct from an empty class selection.
use abos_jbindgen::{Bindings, Builder, ModuleEvent};
use std::{fs, path::PathBuf};
use tempfile::TempDir;
use zip::ZipWriter;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn rejects_invalid_paths_even_alongside_valid_sources() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let java: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	let unsupported: PathBuf = root.path().join("unsupported.txt");
	fs::write(&unsupported, "not Java source")?;
	for invalid in [
		root.path().join("missing-directory"),
		root.path().join("Missing.java"),
		root.path().join("missing.jar"),
		unsupported,
	] {
		for sources in [vec![invalid.clone()], vec![java.clone(), invalid.clone()]] {
			let error: abos_jbindgen::Error = Builder::new()
				.input_sources(sources, Vec::new(), Vec::new())
				.generate()
				.expect_err("invalid source paths must fail");
			assert!(error.to_string().contains(invalid.to_str().ok_or("invalid path")?));
		}
	}
	Ok(())
}

#[test]
fn valid_inputs_with_no_selected_classes_succeed() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let directory: PathBuf = root.path().join("empty");
	fs::create_dir(&directory)?;
	fs::write(directory.join("README.txt"), "no Java sources here")?;
	let jar: PathBuf = root.path().join("empty.jar");
	ZipWriter::new(fs::File::create(&jar)?).finish()?;
	let java: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	for (sources, patterns) in [
		(vec![directory], Vec::new()),
		(vec![jar], Vec::new()),
		(vec![java], vec!["no.match.*".to_owned()]),
	] {
		let bindings: Bindings = Builder::new().input_sources(sources, Vec::new(), patterns).generate()?;
		bindings.walk(&mut |module, event| {
			if event == ModuleEvent::BeginModule {
				assert!(module.bindings.is_empty());
			}
		});
	}
	Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_dangling_symlinks_and_special_files() -> TestResult {
	use std::os::unix::{fs::symlink, net::UnixListener};

	let root: TempDir = tempfile::tempdir()?;
	let dangling: PathBuf = root.path().join("Dangling.java");
	symlink(root.path().join("Missing.java"), &dangling)?;
	let socket: PathBuf = root.path().join("Socket.java");
	let _listener: UnixListener = UnixListener::bind(&socket)?;
	for invalid in [dangling, socket] {
		let error: abos_jbindgen::Error = Builder::new()
			.input_sources(vec![invalid.clone()], Vec::new(), Vec::new())
			.generate()
			.expect_err("source paths must resolve to regular files or directories");
		assert!(error.to_string().contains(invalid.to_str().ok_or("invalid path")?));
	}
	Ok(())
}
