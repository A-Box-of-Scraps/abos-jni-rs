use super::*;
use crate::internal::{
	bytecode::parse_class,
	generator::signatures::{generate_dex_signature, generate_field_dex_signature, type_to_dex_signature},
};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
struct RuntimeApi {
	class_name: String,
	methods: BTreeSet<(String, bool)>,
	fields: BTreeSet<(String, bool, bool)>,
}

fn runtime_api(class: &ClassInfo) -> RuntimeApi {
	// Parameter names, documentation, and source annotations are not JVM signatures.
	// Callers explicitly exclude compiler-generated members absent from the source API.
	RuntimeApi {
		class_name: class.class_name.clone(),
		methods: class
			.constructors
			.iter()
			.chain(&class.methods)
			.chain(&class.native_methods)
			.map(|method| (generate_dex_signature(&class.class_name, method), method.is_static))
			.collect(),
		fields: class
			.fields
			.iter()
			.map(|field| {
				(
					generate_field_dex_signature(&class.class_name, field),
					field.is_static,
					field.is_final,
				)
			})
			.collect(),
	}
}

fn expected_api() -> RuntimeApi {
	let methods = [
		("<init>(Ljava/lang/Number;Ljava/lang/Object;)V", false),
		("echo(Ljava/lang/Number;)Ljava/lang/Number;", false),
		("unbounded(Ljava/lang/Object;)Ljava/lang/Object;", false),
		("echoDependent(Ljava/lang/Number;)Ljava/lang/Number;", false),
		("echoArray([Ljava/lang/Number;)[Ljava/lang/Number;", false),
		("matrix([[Ljava/lang/Object;)[[Ljava/lang/Object;", false),
		("related(Ljava/lang/Number;)Ljava/lang/Number;", false),
		("identity(Ljava/lang/Object;)Ljava/lang/Object;", true),
		("bounded(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", true),
		("recursive(Ljava/lang/Comparable;)Ljava/lang/Comparable;", false),
		("methodArray([[Ljava/lang/Number;)[[Ljava/lang/Number;", false),
		("echoList(Ljava/util/List;)Ljava/util/List;", false),
		("entry(Ljava/util/Map$Entry;)Ljava/util/Map$Entry;", false),
	];
	let fields = [
		"number:Ljava/lang/Number;",
		"value:Ljava/lang/Object;",
		"dependent:Ljava/lang/Number;",
		"numbers:[Ljava/lang/Number;",
		"values:[[Ljava/lang/Object;",
		"list:Ljava/util/List;",
	];
	RuntimeApi {
		class_name: "com/example/GenericTypes".to_owned(),
		methods: methods
			.into_iter()
			.map(|(signature, is_static)| (format!("Lcom/example/GenericTypes;->{signature}"), is_static))
			.collect(),
		fields: fields
			.into_iter()
			.map(|signature| (format!("Lcom/example/GenericTypes;->{signature}"), false, false))
			.collect(),
	}
}

#[test]
fn generic_source_matches_bytecode_runtime_signatures() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/GenericTypes.java");
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let classes: Vec<PathBuf> = abos_javac::Build::new()
		.file(&source)
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_compile()?;
	assert_eq!(classes.len(), 1);
	let bytecode: ClassInfo = parse_class(&std::fs::read(&classes[0])?)?;
	assert_eq!(runtime_api(&bytecode), expected_api());
	let sources: Vec<ClassInfo> = parse_java_sources(std::slice::from_ref(&source), &[], "*")?;
	assert_eq!(sources.len(), 1);
	assert_eq!(runtime_api(&sources[0]), runtime_api(&bytecode));

	// Also check the Java descriptor fields, which Rust's TypeInfo conversion does not consume.
	get_parser_jvm()?.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let paths: JObjectArray<'_, JString<'_>> = create_string_array(env, &[source])?;
		let classpath: JObjectArray<'_, JString<'_>> = create_string_array(env, &[])?;
		let pattern: JString<'_> = env.new_string("*")?;
		let classes: JObjectArray<'_, ClassDescription<'_>> = ParserWrapper::parse(env, paths, classpath, pattern)?;
		let class: ClassDescription<'_> = classes.get_element(env, 0)?;
		check_java_descriptors(env, &class)
	})?;
	Ok(())
}

fn check_java_descriptors(env: &mut Env<'_>, class: &ClassDescription<'_>) -> abos_jni::errors::Result<()> {
	for methods in [class.constructors(env)?, class.methods(env)?] {
		for i in 0..methods.len(env)? {
			let method: MethodDescription<'_> = methods.get_element(env, i)?;
			let info: MethodInfo = java_to_method_info(env, &method)?;
			let result: TypeDescription<'_> = method.return_type(env)?;
			assert_eq!(
				result.descriptor(env)?.to_string(),
				type_to_dex_signature(&info.signature.return_type)
			);
			let parameters: JObjectArray<'_, ParameterDescription<'_>> = method.parameters(env)?;
			for (i, arg) in info.signature.arguments.iter().enumerate() {
				assert_eq!(
					parameters.get_element(env, i)?.descriptor(env)?.to_string(),
					type_to_dex_signature(&arg.type_info)
				);
			}
		}
	}
	let fields: JObjectArray<'_, FieldDescription<'_>> = class.fields(env)?;
	for i in 0..fields.len(env)? {
		let field: FieldDescription<'_> = fields.get_element(env, i)?;
		let info: FieldInfo = java_to_field_info(env, &field)?;
		assert_eq!(
			field.descriptor(env)?.to_string(),
			type_to_dex_signature(&info.type_info)
		);
	}
	Ok(())
}

#[test]
fn inner_class_source_matches_bytecode_constructors() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/EnclosingTypes.java");
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let paths: Vec<PathBuf> = abos_javac::Build::new()
		.file(&source)
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_compile()?;
	let sources: Vec<ClassInfo> = parse_java_sources(&[source], &[], "*")?;
	let expected = [
		("EnclosingTypes", "(I)V"),
		("EnclosingTypes$Inner", "(Lcom/example/EnclosingTypes;I)V"),
		("EnclosingTypes$Inner$Deep", "(Lcom/example/EnclosingTypes$Inner;I)V"),
		("EnclosingTypes$Implicit", "(Lcom/example/EnclosingTypes;)V"),
		("EnclosingTypes$Derived", "(Lcom/example/EnclosingTypes;I)V"),
		("EnclosingTypes$StaticNested", "(I)V"),
		("EnclosingTypes$Container$Member", "(I)V"),
	];
	assert_eq!(sources.len(), expected.len() + 1); // Includes the constructor-less interface.
	assert_eq!(paths.len(), sources.len());
	for (name, descriptor) in expected {
		let name: String = format!("com/example/{name}");
		let source: &ClassInfo = sources
			.iter()
			.find(|class| class.class_name == name)
			.expect("source class");
		let bytes: Vec<u8> = std::fs::read(output.path().join(format!("{name}.class")))?;
		let mut bytecode: ClassInfo = parse_class(&bytes)?;
		// javac emits synthetic enclosing-instance fields absent from the source API.
		// Only ACC_SYNTHETIC (0x1000) fields are excluded from the parity comparison.
		let raw: abos_cafebabe::ClassFile = abos_cafebabe::parse_class(&bytes)?;
		let synthetic: BTreeSet<&str> = raw
			.fields
			.iter()
			.filter(|field| field.access_flags.contains(abos_cafebabe::AccessFlags(0x1000)))
			.map(|field| field.name.as_str())
			.collect();
		bytecode.fields.retain(|field| !synthetic.contains(field.name.as_str()));
		assert_eq!(bytecode.constructors.len(), 1);
		assert_eq!(
			generate_dex_signature(&name, &bytecode.constructors[0]),
			format!("L{name};-><init>{descriptor}")
		);
		assert_eq!(runtime_api(source), runtime_api(&bytecode));
	}
	Ok(())
}

#[test]
fn rejects_selected_local_classes_with_diagnostics() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/LocalClasses.java");
	for pattern in [
		"*",
		"com.example.LocalClasses$1Captured",
		"com.example.LocalClasses$1Captured$Member",
		"com.example.LocalClasses$1Uncaptured",
	] {
		let error: Error = crate::Builder::new()
			.input_sources(vec![source.clone()], vec![], vec![pattern.to_owned()])
			.generate()
			.expect_err("selected local classes must not generate invalid bindings");
		let message: String = error.to_string();
		assert!(message.contains("Unsupported local class"), "{message}");
		assert!(message.contains("com.example.LocalClasses$1"), "{message}");
		assert!(message.contains("class-file input"), "{message}");
	}
	// Unselected implementation-local classes must not prevent supported member generation.
	let bindings: crate::Bindings = crate::Builder::new()
		.input_sources(
			vec![source],
			vec![],
			vec!["com.example.LocalClasses$Supported".to_owned()],
		)
		.generate()?;
	assert_eq!(bindings.len(), 1);
	Ok(())
}

#[test]
fn records_and_annotations_match_bytecode_names() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let fixtures: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java");
	let files: [PathBuf; 2] = [fixtures.join("RecordTypes.java"), fixtures.join("Marker.java")];
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let paths: Vec<PathBuf> = abos_javac::Build::new()
		.files(&files)
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_compile()?;
	let sources: Vec<ClassInfo> = parse_java_sources(&files, &[], "*")?;
	let expected = [
		("RecordTypes", "<init>(I)V"),
		("RecordTypes$Nested", "<init>([Lcom/example/RecordTypes;)V"),
		(
			"RecordTypes$Member",
			"echo(Lcom/example/RecordTypes;)Lcom/example/RecordTypes;",
		),
		("Marker", "value()I"),
		("Marker$Nested", "value()[Lcom/example/Marker;"),
		("Marker$Member", "<init>([Lcom/example/Marker;)V"),
	];
	assert_eq!(sources.len(), expected.len());
	assert_eq!(paths.len(), expected.len());
	for (name, signature) in expected {
		let name: String = format!("com/example/{name}");
		let source: &ClassInfo = sources
			.iter()
			.find(|class| class.class_name == name)
			.unwrap_or_else(|| panic!("missing source class {name}"));
		let bytecode: ClassInfo = parse_class(&std::fs::read(output.path().join(format!("{name}.class")))?)?;
		let api: RuntimeApi = runtime_api(source);
		assert!(api.methods.contains(&(format!("L{name};->{signature}"), false)));
		// Record-generated accessors, equals/hashCode/toString are present in both models.
		// No members are excluded from this comparison.
		assert_eq!(api, runtime_api(&bytecode));
	}
	get_parser_jvm()?.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let paths: JObjectArray<'_, JString<'_>> = create_string_array(env, &files)?;
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

#[test]
fn enum_constants_match_bytecode_runtime_api() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let file: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/EnumTypes.java");
	let output: tempfile::TempDir = tempfile::tempdir()?;
	let paths: Vec<PathBuf> = abos_javac::Build::new()
		.file(&file)
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_compile()?;
	let sources: Vec<ClassInfo> = parse_java_sources(std::slice::from_ref(&file), &[], "*")?;
	assert_eq!(sources.len(), 3);
	// The fourth class is BLUE's anonymous constant body, not a separately named API.
	assert_eq!(paths.len(), 4);
	for (simple_name, constants) in [
		("EnumTypes", &["RED", "BLUE"][..]),
		("EnumTypes$State", &["ON", "OFF"][..]),
		("EnumTypes$Empty", &[][..]),
	] {
		let name: String = format!("com/example/{simple_name}");
		let mut expected = RuntimeApi {
			class_name: name.clone(),
			methods: [
				(format!("L{name};->values()[L{name};"), true),
				(format!("L{name};->valueOf(Ljava/lang/String;)L{name};"), true),
			]
			.into_iter()
			.collect(),
			fields: constants
				.iter()
				.map(|constant| (format!("L{name};->{constant}:L{name};"), true, true))
				.collect(),
		};
		if simple_name == "EnumTypes" {
			expected.methods.insert((format!("L{name};->code()I"), false));
			expected.fields.insert((format!("L{name};->id:I"), false, true));
		}
		let bytecode: ClassInfo = parse_class(&std::fs::read(output.path().join(format!("{name}.class")))?)?;
		assert_eq!(runtime_api(&bytecode), expected);
		let source: &ClassInfo = sources
			.iter()
			.find(|class| class.class_name == name)
			.expect("source enum");
		// No public members are excluded: constants are static final fields of the enum type,
		// even when the value has an anonymous constant-specific subclass.
		assert_eq!(runtime_api(source), expected);
	}
	get_parser_jvm()?.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let paths: JObjectArray<'_, JString<'_>> = create_string_array(env, &[file])?;
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
