//! Compare declared runtime APIs against an independent, exact contract.
mod api;
mod expected;
#[path = "../../../../support/parity.rs"]
mod fixtures;
mod tests;

use super::*;
use api::{RuntimeApi, assert_api, inventory, normalize_bytecode};
use std::{collections::BTreeMap, fs, io::Write as _};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn source_class_and_jar_match_expected_runtime_api() -> TestResult {
	let root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java");
	let sources: Vec<PathBuf> = fixtures::FIXTURES
		.iter()
		.map(|(name, _)| root.join(format!("{name}.java")))
		.collect();
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let paths: Vec<PathBuf> = abos_javac::Build::new()
		.files(&sources)
		.output_dir(output.path())
		.encoding("UTF-8")
		.cargo_metadata(false)
		.try_compile()?;
	let expected: BTreeMap<String, RuntimeApi> = expected::all();
	let source: Vec<ClassInfo> = parse_java_sources(&sources, &[], "*")?;
	assert_api(&inventory(&source), &expected, "source");

	let jar: PathBuf = output.path().join("fixtures.jar");
	let mut archive: zip::ZipWriter<fs::File> = zip::ZipWriter::new(fs::File::create(&jar)?);
	let mut classes: Vec<ClassInfo> = Vec::new();
	let mut anonymous: usize = 0;
	for path in paths {
		let bytes: Vec<u8> = fs::read(&path)?;
		let entry: String = path
			.strip_prefix(output.path())?
			.to_str()
			.ok_or("non-UTF-8 class path")?
			.replace('\\', "/");
		archive.start_file(&entry, zip::write::SimpleFileOptions::default())?;
		archive.write_all(&bytes)?;
		// EnumTypes.BLUE's anonymous body is not a separately named source declaration.
		if entry == "com/example/EnumTypes$1.class" {
			anonymous += 1;
		} else {
			classes.push(normalize_bytecode(&bytes)?);
		}
	}
	archive.finish()?;
	assert_eq!(anonymous, 1, "exact anonymous implementation-class inventory");
	assert_api(&inventory(&classes), &expected, "class files");
	let mut archived: Vec<ClassInfo> = Vec::new();
	crate::internal::inputs::visit_jar_classes(&jar, &[], |name, bytes| {
		if name != "com/example/EnumTypes$1.class" {
			archived.push(normalize_bytecode(bytes)?);
		}
		Ok(())
	})?;
	assert_api(&inventory(&archived), &expected, "JAR");
	check_descriptors(&sources)?;
	Ok(())
}

fn check_descriptors(sources: &[PathBuf]) -> TestResult {
	get_parser_jvm()?.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let paths: JObjectArray<'_, JString<'_>> = create_string_array(env, sources)?;
		let classpath: JObjectArray<'_, JString<'_>> = create_string_array(env, &[])?;
		let pattern: JString<'_> = env.new_string("*")?;
		let classes: JObjectArray<'_, ClassDescription<'_>> = ParserWrapper::parse(env, paths, classpath, pattern)?;
		for i in 0..classes.len(env)? {
			let class: ClassDescription<'_> = classes.get_element(env, i)?;
			check_java_descriptors(env, &class)?;
		}
		Ok(())
	})?;
	Ok(())
}
