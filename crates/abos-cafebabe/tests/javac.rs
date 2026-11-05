//! Integration tests against real compiler output. Requires a JDK 17 or newer.
use abos_cafebabe::{ClassFile, parse_class};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join(name)
}

#[test]
fn compiler_output_across_releases() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	// Keep the supplementary Unicode identifier outside the Java formatter
	// scope: it is valid javac input but unsupported by the formatter.
	for release in ["8", "17"] {
		let classes: Vec<PathBuf> = abos_javac::Build::new()
			.file(fixture("cases/legacy/sample/Legacy.java"))
			.release(release)
			.encoding("UTF-8")
			.output_dir(root.path().join(release))
			.cargo_metadata(false)
			.try_compile()?;
		for path in classes {
			let class: ClassFile = parse_class(&fs::read(path)?)?;
			assert_eq!(class.major_version, if release == "8" { 52 } else { 61 });
			if class.this_class == "sample/Legacy" {
				check_legacy(&class);
			}
		}
	}
	Ok(())
}

fn check_legacy(class: &ClassFile) {
	assert_eq!(class.interfaces, ["java/lang/Runnable"]);
	assert!(class.fields.iter().any(|field| field.name == "𐐀"));
	assert!(
		class
			.methods
			.iter()
			.any(|method| method.name == "call" && method.descriptor == "([I)[[Ljava/lang/String;")
	);
}

#[test]
fn records_sealed_classes_and_modules() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.files([
			fixture("java/modern/sample/Shape.java"),
			fixture("java/modern/sample/Point.java"),
			fixture("java/modern/module-info.java"),
		])
		.output_dir(root.path().join("classes"))
		.cargo_metadata(false)
		.try_compile()?;
	let mut names: Vec<String> = Vec::new();
	for path in classes {
		let class: ClassFile = parse_class(&fs::read(path)?)?;
		if class.this_class == "module-info" {
			assert!(class.super_class.is_none());
		}
		if class.this_class == "sample/Point" {
			assert_eq!(class.super_class.as_deref(), Some("java/lang/Record"));
			assert_eq!(class.fields.len(), 2);
		}
		names.push(class.this_class);
	}
	names.sort();
	assert_eq!(names, ["module-info", "sample/Point", "sample/Shape"]);
	Ok(())
}
