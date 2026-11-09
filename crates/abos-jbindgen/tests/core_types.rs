//! Core classes use built-in JNI wrappers rather than generated declarations.
#[path = "support/core_classes.rs"]
mod core_classes;

use abos_jbindgen::{Bindings, Builder, Error};
use std::{fs, io::Write, path::PathBuf};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn rejects_direct_core_class_inputs() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	for (name, bytes) in core_classes::entries(root.path())? {
		let path: PathBuf = root.path().join("core.class");
		fs::write(&path, &bytes)?;
		for builder in [
			Builder::new().input_class(&path),
			Builder::new().input_class_bytes(bytes),
		] {
			let error: Error = builder
				.generate()
				.expect_err("core declarations must fail during generation");
			assert!(matches!(error, Error::Unsupported(_)), "{error}");
			let java: String = name.trim_end_matches(".class").replace('/', ".");
			assert!(error.to_string().contains(&java), "{error}");
			assert!(error.to_string().contains("abos_jni::objects::J"), "{error}");
		}
	}
	Ok(())
}

#[test]
fn bulk_archives_skip_core_declarations_and_use_builtin_types() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/CoreTypes.java");
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.file(source)
		.output_dir(root.path())
		.cargo_metadata(false)
		.try_compile()?;
	let jar: PathBuf = root.path().join("core.jar");
	let mut archive: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(&jar)?);
	for (name, bytes) in core_classes::entries(root.path())? {
		archive.start_file(name, SimpleFileOptions::default())?;
		archive.write_all(&bytes)?;
	}
	archive.start_file("com/example/CoreTypes.class", SimpleFileOptions::default())?;
	archive.write_all(&fs::read(&classes[0])?)?;
	archive.finish()?;

	for patterns in [vec![], vec!["*"], vec!["**"], vec!["java.lang.*", "com.example.*"]] {
		let bindings: Bindings = Builder::new().input_jar(&jar).patterns(patterns).generate()?;
		assert_eq!(bindings.len(), 1);
		assert_eq!(bindings.type_map(None).len(), 1);
		let code: String = bindings.to_string();
		for builtin in ["JObject", "JClass", "JString", "JThrowable", "JString[]"] {
			assert!(code.contains(builtin), "missing {builtin}: {code}");
		}
		assert!(!code.contains("=> \"java.lang."), "{code}");
	}
	assert!(
		Builder::new()
			.input_jar(&jar)
			.patterns(["java.lang.*"])
			.generate()?
			.is_empty()
	);
	for (name, _) in core_classes::entries(root.path())? {
		let internal: &str = name.trim_end_matches(".class");
		for pattern in [internal.to_owned(), internal.replace('/', ".")] {
			let error: Error = Builder::new()
				.input_jar(&jar)
				.patterns(["*", &pattern])
				.generate()
				.expect_err("exact core requests must fail");
			assert!(matches!(error, Error::Unsupported(_)), "{error}");
		}
	}
	Ok(())
}

#[test]
fn rejects_exact_core_patterns_for_sources_and_android() {
	for pattern in ["java.lang.String", "java/lang/Object", "*, java.lang.Throwable"] {
		for builder in [
			Builder::new().input_sources(Vec::new(), Vec::new(), vec![pattern.to_owned()]),
			Builder::new().input_android_sdk(35, vec![pattern.to_owned()]),
		] {
			let error: Error = builder
				.generate()
				.expect_err("explicit core requests must fail before parsing or SDK discovery");
			assert!(matches!(error, Error::Unsupported(_)), "{error}");
		}
	}
}
