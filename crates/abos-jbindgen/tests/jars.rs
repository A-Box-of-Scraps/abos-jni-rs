//! JAR metadata filtering and deterministic base-only multi-release selection.
#[path = "support/jars.rs"]
mod fixtures;
use abos_jbindgen::{Bindings, Builder};
use fixtures::{Fixtures, TestResult};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn skips_metadata_and_selects_only_base_classes_in_archive_name_order() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let sources: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/archives");
	let jars: Fixtures = fixtures::create(&sources, root.path())?;
	let modular: Bindings = Builder::new().input_jar(&jars.modular).generate()?;
	assert_eq!(modular.len(), 1);
	assert!(modular.to_string().contains("pub ModularExample"));
	let base: String = Builder::new().input_jar(&jars.multi_release).generate()?.to_string();
	let reversed: String = Builder::new().input_jar(&jars.reversed).generate()?.to_string();
	assert_eq!(base, reversed);
	assert!(base.contains("fn base_only("), "{base}");
	for excluded in ["module_info", "package_info", "fn version_only", "pub VersionOnly"] {
		assert!(!base.contains(excluded), "unexpected {excluded}: {base}");
	}
	assert_eq!(base.matches("pub VersionedExample =>").count(), 1);
	Ok(())
}

#[test]
fn patterns_use_base_class_names_and_retain_inner_classes() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let sources: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/archives");
	let jars: Fixtures = fixtures::create(&sources, root.path())?;
	let builder: Builder = Builder::new().input_jar(&jars.multi_release);
	let bindings: Bindings = builder
		.clone()
		.patterns(["fixture.archive.VersionedExample"])
		.generate()?;
	assert_eq!(bindings.len(), 2);
	assert!(bindings.to_string().contains("pub VersionedExampleInner"));
	assert!(!bindings.to_string().contains("pub ModularExample"));
	for pattern in [
		"META-INF.versions.*",
		"fixture.archive.VersionOnly",
		"fixture.archive.package-info",
		"module-info",
	] {
		assert!(builder.clone().patterns([pattern]).generate()?.is_empty(), "{pattern}");
	}
	Ok(())
}

#[test]
fn excluded_entries_are_not_parsed_even_when_patterns_match_them() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let path: PathBuf = root.path().join("metadata.jar");
	let entries: Vec<(String, Vec<u8>)> = [
		"module-info.class",
		"sample/package-info.class",
		"META-INF/versions/17/sample/Example.class",
		"META-INF/Example.class",
		"assets/data.txt",
	]
	.into_iter()
	.map(|name| (name.to_owned(), b"not a class file".to_vec()))
	.collect();
	fixtures::write_jar(&path, &entries)?;
	for patterns in [vec![], vec!["**"], vec!["META-INF.versions.*"], vec!["sample.*"]] {
		assert!(
			Builder::new()
				.input_jar(&path)
				.patterns(patterns)
				.generate()?
				.is_empty()
		);
	}
	Ok(())
}

#[test]
fn invalid_selected_base_classes_still_report_errors() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let path: PathBuf = root.path().join("broken.jar");
	fixtures::write_jar(
		&path,
		&[("sample/Broken.class".to_owned(), b"not a class file".to_vec())],
	)?;
	let builder: Builder = Builder::new().input_jar(path);
	assert!(builder.clone().patterns(["other.*"]).generate()?.is_empty());
	let error: String = builder.generate().expect_err("invalid selected class").to_string();
	assert!(error.contains("Failed to parse sample/Broken.class"), "{error}");
	Ok(())
}
