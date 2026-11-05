//! End-to-end binding generation from Java source, bytecode, and archives.
mod support;
use abos_jbindgen::{Bindings, Builder, Error, ModuleEvent, TypeMap};
use std::{fs, io::Write, path::PathBuf};
use support::{TestResult, annotations, fixture, source_builder};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

#[test]
fn supports_source_class_bytes_and_jar_inputs() -> TestResult {
	let root: TempDir = tempfile::tempdir()?;
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.file(fixture("SimpleClass.java"))
		.output_dir(root.path())
		.cargo_metadata(false)
		.try_compile()?;
	let bytes: Vec<u8> = fs::read(&classes[0])?;
	let jar: PathBuf = root.path().join("fixture.jar");
	let mut archive: ZipWriter<fs::File> = ZipWriter::new(fs::File::create(&jar)?);
	archive.start_file("com/example/SimpleClass.class", SimpleFileOptions::default())?;
	archive.write_all(&bytes)?;
	archive.finish()?;
	for builder in [
		source_builder(&["SimpleClass.java"]),
		Builder::new().input_class(&classes[0]),
		Builder::new().input_class_bytes(bytes),
		Builder::new().input_jar(jar),
	] {
		let bindings: Bindings = builder.generate()?;
		let code: String = bindings.to_string();
		assert_eq!(bindings.len(), 1);
		for fragment in [
			"pub SimpleClass",
			"fn new(",
			"fn add(",
			"fn get_value(",
			"pub fn jni_init",
		] {
			assert!(code.contains(fragment), "missing {fragment}: {code}");
		}
	}
	Ok(())
}

#[test]
fn writes_single_file_tree_and_type_maps() -> TestResult {
	let bindings: Bindings = source_builder(&[
		"NoPackage.java",
		"com/example/data/Person.java",
		"com/example/service/PersonService.java",
	])
	.root_path("crate::bindings")
	.type_mapping("external::Type", "external.Type")
	.generate()?;
	assert!(!bindings.is_empty());
	let mut events: Vec<(String, ModuleEvent)> = Vec::new();
	bindings.walk(&mut |module, event| events.push((module.name.clone(), event)));
	assert_eq!(
		events
			.iter()
			.filter(|(_, event)| *event == ModuleEvent::BeginModule)
			.count(),
		5
	);
	let root: TempDir = tempfile::tempdir()?;
	let file: PathBuf = root.path().join("bindings.rs");
	bindings.write_to_file(&file)?;
	assert_eq!(fs::read_to_string(file)?, bindings.to_string());
	bindings.write_to_files(root.path().join("modules"))?;
	assert!(root.path().join("modules/com/example/data/mod.rs").is_file());
	assert!(fs::read_to_string(root.path().join("modules/mod.rs"))?.contains("NoPackage"));
	bindings.write_type_map(root.path().join("internal.txt"))?;
	bindings.write_pub_type_map(root.path().join("public.txt"), "api")?;
	let map: TypeMap = bindings.type_map(Some("api"));
	assert_eq!(
		map.get_rust_type("com.example.data.Person"),
		Some("api::com::example::data::Person")
	);
	assert_eq!(map.get_rust_type("external.Type"), Some("external::Type"));
	assert!(fs::read_to_string(root.path().join("internal.txt"))?.contains("crate::bindings"));
	assert!(!fs::read_to_string(root.path().join("public.txt"))?.contains("crate::bindings"));
	Ok(())
}

#[test]
fn handles_overloads_fields_inner_classes_and_native_visibility() -> TestResult {
	for (file, fragments) in [
		(
			"Overloads.java",
			&["fn new_int", "fn new_string", "fn process_int", "fn process_string"][..],
		),
		(
			"OuterClass.java",
			&["pub OuterClass", "pub OuterClassInnerClass", "$InnerClass"][..],
		),
		("NativeMethodVisibility.java", &["native_methods {", "pub fn"][..]),
		("WithFields.java", &["fields {", "static", "get ="][..]),
	] {
		let code: String = source_builder(&[file]).generate()?.to_string();
		for fragment in fragments {
			assert!(code.contains(fragment), "{file} missing {fragment}: {code}");
		}
	}
	let code: String = source_builder(&["NativeMethodVisibility.java"])
		.generate_native_interfaces(false)
		.generate()?
		.to_string();
	assert!(!code.contains("native_methods {"));
	Ok(())
}

#[test]
fn honors_configuration_skips_and_name_overrides() -> TestResult {
	let bindings: Bindings = source_builder(&["SimpleClass.java"])
		.private_type(true)
		.name_prefix("Java")
		.generate_jni_init(false)
		.skip_signature("Lcom/example/SimpleClass;->getValue()I")
		.name_override("Lcom/example/SimpleClass;->setValue(I)V", "assign_value")
		.generate()?;
	let code: String = bindings.to_string();
	assert!(code.contains("JavaSimpleClass =>"));
	assert!(!code.contains("pub JavaSimpleClass"));
	assert!(!code.contains("get_value"));
	assert!(code.contains("assign_value"));
	assert!(!code.contains("pub fn jni_init"));
	Ok(())
}

#[test]
fn annotations_are_loaded_from_exported_and_compiled_sources() -> TestResult {
	let annotations: TempDir = annotations()?;
	for (file, included, excluded) in [
		(
			"AnnotationTest.java",
			"pub CustomAnnotatedClass",
			"pub AnnotationTest =>",
		),
		("RustSkipTest.java", "included_method", "skipped_method"),
	] {
		let code: String = Builder::new()
			.input_sources(vec![fixture(file)], vec![annotations.path().to_owned()], Vec::new())
			.generate()?
			.to_string();
		assert!(code.contains(included), "{code}");
		assert!(!code.contains(excluded), "{code}");
	}
	Ok(())
}

#[test]
fn primitive_annotations_require_matching_mappings() -> TestResult {
	let annotations: TempDir = annotations()?;
	let builder: Builder = Builder::new().input_sources(
		vec![fixture("RustPrimitiveTest.java")],
		vec![annotations.path().to_owned()],
		Vec::new(),
	);
	assert!(matches!(builder.clone().generate(), Err(Error::CodeGen(_))));
	let code: String = builder
		.clone()
		.type_mapping("unsafe ThingHandle", "long")
		.type_mapping("unsafe OtherHandle", "long")
		.generate()?
		.to_string();
	assert!(code.contains("unsafe ThingHandle => long"));
	assert!(code.contains("handle: ThingHandle"));
	assert!(builder.type_mapping("unsafe ThingHandle", "int").generate().is_err());
	Ok(())
}

#[test]
fn rejects_missing_or_invalid_inputs() {
	for builder in [
		Builder::new(),
		Builder::new().input_class_bytes(vec![0, 1, 2]),
		Builder::new().input_class("missing.class"),
	] {
		assert!(builder.generate().is_err());
	}
}

#[test]
fn source_generation_is_repeatable_and_preserves_docs() -> TestResult {
	let first: String = source_builder(&["WithDocumentation.java"]).generate()?.to_string();
	assert!(first.contains("/// * `initial_value`"));
	assert!(first.contains("#[allow(non_snake_case, reason = "));
	for _ in 0..3 {
		assert_eq!(
			first,
			source_builder(&["WithDocumentation.java"]).generate()?.to_string()
		);
	}
	Ok(())
}

#[test]
fn type_map_protects_core_jni_types() {
	let mut map = TypeMap::new();
	map.insert("java.lang.String".to_owned(), "BadString".to_owned());
	map.merge(vec![("BadObject".to_owned(), "java.lang.Object".to_owned())]);
	assert!(map.is_empty());
	map.insert("app.Model".to_owned(), "Model".to_owned());
	assert_eq!(map.len(), 1);
	assert_eq!(map.get_rust_type("app.Model"), Some("Model"));
}

#[test]
fn skipped_members_do_not_reserve_rust_names() -> TestResult {
	let code: String = source_builder(&["MemberCollisions.java"])
		.skip_signature("Lcom/example/MemberCollisions;-><init>(I)V")
		.skip_signature("Lcom/example/MemberCollisions;-><init>(Ljava/lang/String;)V")
		.skip_signature("Lcom/example/MemberCollisions;->newInt:I")
		.generate()?
		.to_string();
	assert!(!code.contains("constructors {"), "{code}");
	assert!(code.contains("fn new_int() -> jint"), "{code}");
	assert!(code.contains("fn set_new_int_(value: jint) -> jint"), "{code}");
	Ok(())
}

#[test]
fn configured_constructor_names_take_precedence_over_annotations() -> TestResult {
	let annotations: TempDir = annotations()?;
	let root: TempDir = tempfile::tempdir()?;
	let source: PathBuf = root.path().join("ConstructorNames.java");
	fs::write(
		&source,
		"import io.github.jni_rs.jbindgen.RustName;
		public class ConstructorNames {
			@RustName(\"from_annotation\") public ConstructorNames() {}
		}",
	)?;
	let builder: Builder = Builder::new().input_sources(vec![source], vec![annotations.path().to_owned()], Vec::new());
	let annotated: String = builder.clone().generate()?.to_string();
	assert!(annotated.contains("fn from_annotation("), "{annotated}");
	let configured: String = builder
		.name_override("LConstructorNames;-><init>()V", "from_option")
		.generate()?
		.to_string();
	assert!(configured.contains("fn from_option("), "{configured}");
	assert!(!configured.contains("fn from_annotation("), "{configured}");
	Ok(())
}
