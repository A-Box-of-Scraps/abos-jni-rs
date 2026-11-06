//! Identifier normalization, collision handling, and Java lookup names.
mod support;
use abos_jbindgen::{Bindings, Builder, TypeMap};
use std::{fs, path::PathBuf};
use support::{TestResult, fixture, source_builder};
use tempfile::TempDir;

#[test]
fn source_and_bytecode_preserve_java_names_with_normalized_rust_paths() -> TestResult {
	let sources: Vec<PathBuf> = [fixture("Identifiers.java"), fixture("keywords/match.java")].into();
	let root: TempDir = tempfile::tempdir()?;
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.files(&sources)
		.output_dir(root.path())
		.cargo_metadata(false)
		.try_compile()?;
	let source: Bindings = source_builder(&["Identifiers.java", "keywords/match.java"])
		.root_path("crate::bindings")
		.generate()?;
	let map: TypeMap = source.type_map(None);
	assert_eq!(
		map.get_rust_type("com.type.match"),
		Some("crate::bindings::com::type_::match_")
	);
	assert!(map.get_rust_type("com.type_.match_").is_none());
	let code: String = source.to_string();
	assert!(
		code.contains("gen_: jint, foo_bar: jint, foo_bar_: jint, env_: jint, api_: jint"),
		"{code}"
	);
	let class: &PathBuf = classes
		.iter()
		.find(|path| path.ends_with("Identifiers.class"))
		.expect("class");
	for bindings in [source, Builder::new().input_class(class).generate()?] {
		let code: String = bindings.to_string();
		for expected in [
			"pub mod type_",
			"name = \"type\"",
			"name = \"match\"",
			"name = \"gen\"",
			"name = \"λ\"",
		] {
			assert!(code.contains(expected), "missing {expected}: {code}");
		}
	}
	Ok(())
}

#[test]
fn normalizes_explicit_type_member_and_constructor_overrides() -> TestResult {
	let code: String = source_builder(&["SimpleClass.java"])
		.rust_type_name("type")
		.name_override("Lcom/example/SimpleClass;-><init>()V", "gen")
		.name_override("Lcom/example/SimpleClass;->getValue()I", "with-dash")
		.generate()?
		.to_string();
	for expected in ["pub type_ =>", "fn gen_(", "fn with_dash", "name = \"getValue\""] {
		assert!(code.contains(expected), "missing {expected}: {code}");
	}
	Ok(())
}

#[test]
fn dollar_member_names_are_preserved_in_source_and_bytecode_bindings() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	abos_javac::Build::new()
		.file(fixture("UnsupportedNames.java"))
		.output_dir(root.path())
		.cargo_metadata(false)
		.try_compile()?;
	for builder in [
		source_builder(&["UnsupportedNames.java"]),
		Builder::new().input_class(root.path().join("com/example/UnsupportedNames.class")),
	] {
		let code: String = builder.clone().generate()?.to_string();
		for expected in ["name = \"$value\"", "name = \"foo$bar\"", "fn foo_bar"] {
			assert!(code.contains(expected), "missing {expected}: {code}");
		}
		let skipped: String = builder
			.skip_signature("Lcom/example/UnsupportedNames;->$value:I")
			.skip_signature("Lcom/example/UnsupportedNames;->foo$bar()I")
			.generate()?
			.to_string();
		assert!(!skipped.contains("$value"));
		assert!(!skipped.contains("foo$bar"));
	}
	Ok(())
}

#[test]
fn conflicting_normalized_type_names_are_rejected() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let first: PathBuf = root.path().join("type.java");
	let second: PathBuf = root.path().join("type_.java");
	fs::write(&first, "package sample; public class type {}")?;
	fs::write(&second, "package sample; public class type_ {}")?;
	let error: String = Builder::new()
		.input_sources(vec![first, second], Vec::new(), Vec::new())
		.generate()
		.expect_err("conflicting Rust type names")
		.to_string();
	assert!(error.contains("same Rust type"), "{error}");
	Ok(())
}

#[test]
fn normalizes_annotation_overrides() -> TestResult {
	let annotations: TempDir = support::annotations()?;
	let root: TempDir = tempfile::tempdir()?;
	let source: PathBuf = root.path().join("Annotated.java");
	fs::write(
		&source,
		"import io.github.jni_rs.jbindgen.RustName;
		@RustName(\"type\") public class Annotated {
			@RustName(\"gen\") public Annotated() {}
			@RustName(\"with-dash\") public int answer() { return 42; }
		}",
	)?;
	let bindings: Bindings = Builder::new()
		.input_sources(vec![source], vec![annotations.path().to_owned()], Vec::new())
		.generate()?;
	let code: String = bindings.to_string();
	assert!(code.contains("pub type_ =>"), "{code}");
	assert!(code.contains("fn gen_("), "{code}");
	assert!(code.contains("fn with_dash"), "{code}");
	Ok(())
}
