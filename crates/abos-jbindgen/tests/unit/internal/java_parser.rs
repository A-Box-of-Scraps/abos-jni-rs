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
	// This fixture has no compiler-generated bridge/synthetic members to filter out.
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
	let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/GenericTypes.java");
	let output = tempfile::tempdir()?;
	let classes = abos_javac::Build::new()
		.file(&source)
		.output_dir(output.path())
		.cargo_metadata(false)
		.try_compile()?;
	assert_eq!(classes.len(), 1);
	let bytecode = parse_class(&std::fs::read(&classes[0])?)?;
	assert_eq!(runtime_api(&bytecode), expected_api());
	let sources = parse_java_sources(std::slice::from_ref(&source), &[], "*")?;
	assert_eq!(sources.len(), 1);
	assert_eq!(runtime_api(&sources[0]), runtime_api(&bytecode));

	// Also check the Java descriptor fields, which Rust's TypeInfo conversion does not consume.
	get_parser_jvm()?.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let paths = create_string_array(env, &[source])?;
		let classpath = create_string_array(env, &[])?;
		let pattern = env.new_string("*")?;
		let classes = ParserWrapper::parse(env, paths, classpath, pattern)?;
		let class = classes.get_element(env, 0)?;
		check_java_descriptors(env, &class)
	})?;
	Ok(())
}

fn check_java_descriptors(env: &mut Env<'_>, class: &ClassDescription<'_>) -> abos_jni::errors::Result<()> {
	for methods in [class.constructors(env)?, class.methods(env)?] {
		for i in 0..methods.len(env)? {
			let method = methods.get_element(env, i)?;
			let info = java_to_method_info(env, &method)?;
			let result = method.return_type(env)?;
			assert_eq!(
				result.descriptor(env)?.to_string(),
				type_to_dex_signature(&info.signature.return_type)
			);
			let parameters = method.parameters(env)?;
			for (i, arg) in info.signature.arguments.iter().enumerate() {
				assert_eq!(
					parameters.get_element(env, i)?.descriptor(env)?.to_string(),
					type_to_dex_signature(&arg.type_info)
				);
			}
		}
	}
	let fields = class.fields(env)?;
	for i in 0..fields.len(env)? {
		let field = fields.get_element(env, i)?;
		let info = java_to_field_info(env, &field)?;
		assert_eq!(
			field.descriptor(env)?.to_string(),
			type_to_dex_signature(&info.type_info)
		);
	}
	Ok(())
}
