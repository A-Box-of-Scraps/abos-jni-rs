//! Java source parser using `JavacTask` for extracting API information
//!
//! This module provides integration with the Java compiler API to parse
//! Java source files and extract API information including fully-qualified
//! types and parameter names.

use crate::api::error::{Error, Result};
use crate::api::types::{ArgInfo, ClassInfo, FieldInfo, InstanceOfInfo, MethodInfo, MethodSignature, TypeInfo};
use abos_jni::objects::{JObjectArray, JString};
use abos_jni::{Env, InitArgsBuilder, JNIVersion, JavaVM};
use std::path::PathBuf;
use std::sync::OnceLock;

use std::io::Read;

abos_jni::bind_java_type! {
	rust_type = TypeDescription,
	java_type = "com.jbindgen.Parser$TypeDescription",
	fields {
		type_name: JString,
		descriptor: JString,
		array_dimensions: jint,
		is_primitive: jboolean,
	},
}

abos_jni::bind_java_type! {
	rust_type = ParameterDescription,
	java_type = "com.jbindgen.Parser$ParameterDescription",
	type_map = {
		TypeDescription => com.jbindgen.Parser::TypeDescription,
	},
	fields {
		name: JString,
		type_name: JString,
		descriptor: JString,
		array_dimensions: jint,
		is_primitive: jboolean,
		rust_primitive: JString,
	},
}

abos_jni::bind_java_type! {
	rust_type = MethodDescription,
	java_type = "com.jbindgen.Parser$MethodDescription",
	type_map = {
		ParameterDescription => com.jbindgen.Parser::ParameterDescription,
		TypeDescription => com.jbindgen.Parser::TypeDescription,
	},
	fields {
		name: JString,
		documentation: JString,
		rust_name: JString,
		is_static: jboolean,
		is_constructor: jboolean,
		is_native: jboolean,
		is_deprecated: jboolean,
		is_public: jboolean,
		parameters: ParameterDescription[],
		return_type: TypeDescription,
	},
}

abos_jni::bind_java_type! {
	rust_type = FieldDescription,
	java_type = "com.jbindgen.Parser$FieldDescription",
	fields {
		name: JString,
		documentation: JString,
		rust_name: JString,
		type_name: JString,
		descriptor: JString,
		array_dimensions: jint,
		is_primitive: jboolean,
		is_static: jboolean,
		is_final: jboolean,
		is_deprecated: jboolean,
	},
}

abos_jni::bind_java_type! {
	rust_type = ClassDescription,
	java_type = "com.jbindgen.Parser$ClassDescription",
	type_map = {
		MethodDescription => com.jbindgen.Parser::MethodDescription,
		FieldDescription => com.jbindgen.Parser::FieldDescription,
	},
	fields {
		class_name: JString,
		package_name: JString,
		simple_name: JString,
		documentation: JString,
		rust_name: JString,
		constructors: MethodDescription[],
		methods: MethodDescription[],
		fields: FieldDescription[],
		native_methods: MethodDescription[],
		super_class: JString,
		interfaces: JString[],
	},
}

abos_jni::bind_java_type! {
	rust_type = ParserWrapper,
	java_type = "com.jbindgen.ParserWrapper",
	type_map = {
		ClassDescription => com.jbindgen.Parser::ClassDescription,
	},
	methods {
		static fn parse(source_paths: JString[], class_path_entries: JString[], class_pattern: JString) -> ClassDescription[],
	},
}

const PARSER_JAR_BYTES: &[u8] = include_bytes!(env!("JBINDGEN_PARSER_JAR"));

fn load_classes_from_jar(env: &mut Env, jar_bytes: &[u8]) -> std::result::Result<(), abos_jni::errors::Error> {
	use std::io::Cursor;
	use zip::ZipArchive;

	let cursor: std::io::Cursor<&[u8]> = Cursor::new(jar_bytes);
	let mut archive: zip::ZipArchive<std::io::Cursor<&[u8]>> = ZipArchive::new(cursor)
		.map_err(|e| abos_jni::errors::Error::ParseFailed(format!("Failed to read embedded JAR: {e}")))?;

	let class_loader: abos_jni::objects::JClassLoader<'_> =
		abos_jni::objects::JClassLoader::get_system_class_loader(env)?;

	for i in 0..archive.len() {
		let mut file: zip::read::ZipFile<'_, std::io::Cursor<&[u8]>> = archive
			.by_index(i)
			.map_err(|e| abos_jni::errors::Error::ParseFailed(format!("Failed to read JAR entry: {e}")))?;

		let name: String = file
			.name()
			.map_err(|e| abos_jni::errors::Error::ParseFailed(format!("Failed to read JAR entry name: {e}")))?
			.into_owned();

		if std::path::Path::new(&name)
			.extension()
			.is_some_and(|ext| ext == "class")
			&& !name.starts_with("META-INF/")
		{
			let mut class_bytes: Vec<u8> = Vec::new();
			file.read_to_end(&mut class_bytes)
				.map_err(|e| abos_jni::errors::Error::ParseFailed(format!("Failed to read class bytes: {e}")))?;

			let class_name_str: &str = name.trim_end_matches(".class");
			let class_name = abos_jni::strings::JNIString::new(class_name_str);

			let _ = env.define_class(Some(&class_name), &class_loader, &class_bytes)?;
		}
	}

	Ok(())
}

fn get_parser_jvm() -> Result<&'static JavaVM> {
	static JVM: OnceLock<std::result::Result<JavaVM, String>> = OnceLock::new();
	JVM.get_or_init(|| {
		let jvm: abos_jni::vm::JavaVM = if let Ok(existing) = JavaVM::singleton() {
			existing
		} else {
			let args: abos_jni::vm::InitArgs<'_> = InitArgsBuilder::new()
				.version(JNIVersion::V1_8)
				.build()
				.map_err(|error| error.to_string())?;
			JavaVM::new(args).map_err(|error| error.to_string())?
		};
		jvm.attach_current_thread(|env| load_classes_from_jar(env, PARSER_JAR_BYTES))
			.map_err(|error| error.to_string())?;
		Ok(jvm)
	})
	.as_ref()
	.map_err(|message| Error::Parse(message.clone()))
}

/// Parse Java source files using the Java parser
pub fn parse_java_sources(
	source_paths: &[PathBuf],
	class_path_entries: &[PathBuf],
	class_pattern: &str,
) -> Result<Vec<ClassInfo>> {
	let jvm: &abos_jni::vm::JavaVM = get_parser_jvm()?;

	let result: std::result::Result<Vec<ClassInfo>, abos_jni::errors::Error> = jvm.attach_current_thread(|env| {
		let source_paths_array: abos_jni::objects::JObjectArray<'_, abos_jni::objects::JString<'_>> =
			create_string_array(env, source_paths)?;

		let classpath_array: abos_jni::objects::JObjectArray<'_, abos_jni::objects::JString<'_>> =
			create_string_array(env, class_path_entries)?;

		let pattern_jstring: abos_jni::objects::JString<'_> = JString::from_str(env, class_pattern)?;

		let result_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::ClassDescription<'_>> =
			ParserWrapper::parse(env, &source_paths_array, &classpath_array, &pattern_jstring)?;

		let mut classes: Vec<crate::ClassInfo> = Vec::new();
		let len: usize = result_array.len(env)?;
		for i in 0..len {
			let class_desc: crate::internal::java_parser::ClassDescription<'_> = result_array.get_element(env, i)?;
			let class_info: crate::ClassInfo = java_to_class_info(env, &class_desc)?;
			classes.push(class_info);
		}

		Ok(classes)
	});

	result.map_err(|e| Error::Parse(format!("JNI error: {e}")))
}

/// Create a `JString` array from `PathBuf` slice
fn create_string_array<'a>(
	env: &mut Env<'a>,
	paths: &[PathBuf],
) -> std::result::Result<JObjectArray<'a, JString<'a>>, abos_jni::errors::Error> {
	let array: abos_jni::objects::JObjectArray<'_, abos_jni::objects::JString<'_>> =
		JObjectArray::<JString>::new(env, paths.len(), JString::null())?;
	for (i, path) in paths.iter().enumerate() {
		let path_str: &str = path
			.to_str()
			.ok_or_else(|| abos_jni::errors::Error::ParseFailed("Invalid path encoding".to_string()))?;
		let jstring: abos_jni::objects::JString<'_> = JString::from_str(env, path_str)?;
		array.set_element(env, i, &jstring)?;
	}

	Ok(array)
}

/// Convert Java `ClassDescription` to `ClassInfo`
fn java_to_class_info(
	env: &mut Env,
	class_desc: &ClassDescription,
) -> std::result::Result<ClassInfo, abos_jni::errors::Error> {
	let class_name: String = class_desc.class_name(env)?.to_string();
	let package_name: String = class_desc.package_name(env)?.to_string();
	let simple_name: String = class_desc.simple_name(env)?.to_string();

	let package: Vec<String> = if package_name.is_empty() {
		Vec::new()
	} else {
		package_name.split('.').map(std::string::ToString::to_string).collect()
	};

	let constructors_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::MethodDescription<'_>> =
		class_desc.constructors(env)?;
	let constructors: Vec<crate::MethodInfo> = java_array_to_methods(env, &constructors_array)?;

	let methods_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::MethodDescription<'_>> =
		class_desc.methods(env)?;
	let methods: Vec<crate::MethodInfo> = java_array_to_methods(env, &methods_array)?;

	let fields_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::FieldDescription<'_>> =
		class_desc.fields(env)?;
	let fields: Vec<crate::FieldInfo> = java_array_to_fields(env, &fields_array)?;

	let native_methods_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::MethodDescription<'_>> =
		class_desc.native_methods(env)?;
	let native_methods: Vec<crate::MethodInfo> = java_array_to_methods(env, &native_methods_array)?;

	let mut instance_of: Vec<crate::InstanceOfInfo> = Vec::new();

	let super_class: String = class_desc.super_class(env)?.to_string();
	if !super_class.is_empty() && super_class != "java.lang.Object" {
		instance_of.push(InstanceOfInfo {
			java_type: super_class,
			stem: None,
		});
	}

	let interfaces_array: abos_jni::objects::JObjectArray<'_, abos_jni::objects::JString<'_>> =
		class_desc.interfaces(env)?;
	let len: usize = interfaces_array.len(env)?;
	for i in 0..len {
		let interface_jstring: abos_jni::objects::JString<'_> = interfaces_array.get_element(env, i)?;
		let interface: String = interface_jstring.to_string();
		instance_of.push(InstanceOfInfo {
			java_type: interface,
			stem: None,
		});
	}

	Ok(ClassInfo {
		class_name,
		package,
		simple_name,
		documentation: Some(class_desc.documentation(env)?.to_string()),
		rust_name_override: {
			let rust_name_jstring: abos_jni::objects::JString<'_> = class_desc.rust_name(env)?;
			if rust_name_jstring.is_null() {
				None
			} else {
				Some(rust_name_jstring.to_string())
			}
		},
		constructors,
		methods,
		fields,
		native_methods,
		instance_of,
	})
}

/// Convert Java `MethodDescription` array to Vec<MethodInfo>
fn java_array_to_methods(
	env: &mut Env,
	array: &JObjectArray<MethodDescription>,
) -> std::result::Result<Vec<MethodInfo>, abos_jni::errors::Error> {
	let len: usize = array.len(env)?;
	let mut methods: Vec<crate::MethodInfo> = Vec::with_capacity(len);

	for i in 0..len {
		let method_desc: crate::internal::java_parser::MethodDescription<'_> = array.get_element(env, i)?;
		let method_info: crate::MethodInfo = java_to_method_info(env, &method_desc)?;
		methods.push(method_info);
	}

	Ok(methods)
}

/// Convert Java `MethodDescription` to `MethodInfo`
fn java_to_method_info(
	env: &mut Env,
	method_desc: &MethodDescription,
) -> std::result::Result<MethodInfo, abos_jni::errors::Error> {
	let name: String = method_desc.name(env)?.to_string();
	let documentation: String = method_desc.documentation(env)?.to_string();
	let is_static: bool = method_desc.is_static(env)?;
	let is_constructor: bool = method_desc.is_constructor(env)?;
	let is_native: bool = method_desc.is_native(env)?;
	let is_deprecated: bool = method_desc.is_deprecated(env)?;
	let is_public: bool = method_desc.is_public(env)?;

	let params_array: abos_jni::objects::JObjectArray<'_, crate::internal::java_parser::ParameterDescription<'_>> =
		method_desc.parameters(env)?;
	let len: usize = params_array.len(env)?;
	let mut arguments: Vec<crate::ArgInfo> = Vec::with_capacity(len);

	for i in 0..len {
		let param_desc: crate::internal::java_parser::ParameterDescription<'_> = params_array.get_element(env, i)?;
		let arg_info: crate::ArgInfo = java_to_arg_info(env, &param_desc)?;
		arguments.push(arg_info);
	}

	let return_type_desc: crate::internal::java_parser::TypeDescription<'_> = method_desc.return_type(env)?;
	let return_type: crate::TypeInfo = java_to_type_info(env, &return_type_desc)?;

	Ok(MethodInfo {
		name,
		documentation: Some(documentation),
		rust_name_override: {
			let rust_name_jstring: abos_jni::objects::JString<'_> = method_desc.rust_name(env)?;
			if rust_name_jstring.is_null() {
				None
			} else {
				Some(rust_name_jstring.to_string())
			}
		},
		signature: MethodSignature { arguments, return_type },
		is_static,
		is_constructor,
		is_native,
		is_deprecated,
		is_public,
	})
}

/// Convert Java `ParameterDescription` to `ArgInfo`
fn java_to_arg_info(
	env: &mut Env,
	param_desc: &ParameterDescription,
) -> std::result::Result<ArgInfo, abos_jni::errors::Error> {
	let name: String = param_desc.name(env)?.to_string();
	let type_name: String = param_desc.type_name(env)?.to_string();
	let array_dimensions: usize = usize::try_from(param_desc.array_dimensions(env)?)
		.map_err(|error| abos_jni::errors::Error::ParseFailed(error.to_string()))?;
	let is_primitive: bool = param_desc.is_primitive(env)?;

	let rust_primitive_jstring: abos_jni::objects::JString<'_> = param_desc.rust_primitive(env)?;
	let rust_primitive: Option<String> = if rust_primitive_jstring.is_null() {
		None
	} else {
		Some(rust_primitive_jstring.to_string())
	};

	Ok(ArgInfo {
		name: Some(name),
		type_info: TypeInfo {
			name: type_name,
			array_dimensions,
			is_primitive,
		},
		rust_primitive,
	})
}

/// Convert Java `TypeDescription` to `TypeInfo`
fn java_to_type_info(
	env: &mut Env,
	type_desc: &TypeDescription,
) -> std::result::Result<TypeInfo, abos_jni::errors::Error> {
	let type_name: String = type_desc.type_name(env)?.to_string();
	let array_dimensions: usize = usize::try_from(type_desc.array_dimensions(env)?)
		.map_err(|error| abos_jni::errors::Error::ParseFailed(error.to_string()))?;
	let is_primitive: bool = type_desc.is_primitive(env)?;

	Ok(TypeInfo {
		name: type_name,
		array_dimensions,
		is_primitive,
	})
}

/// Convert Java `FieldDescription` array to Vec<FieldInfo>
fn java_array_to_fields(
	env: &mut Env,
	array: &JObjectArray<FieldDescription>,
) -> std::result::Result<Vec<FieldInfo>, abos_jni::errors::Error> {
	let len: usize = array.len(env)?;
	let mut fields: Vec<crate::FieldInfo> = Vec::with_capacity(len);

	for i in 0..len {
		let field_desc: crate::internal::java_parser::FieldDescription<'_> = array.get_element(env, i)?;
		let field_info: crate::FieldInfo = java_to_field_info(env, &field_desc)?;
		fields.push(field_info);
	}

	Ok(fields)
}

/// Convert Java `FieldDescription` to `FieldInfo`
fn java_to_field_info(
	env: &mut Env,
	field_desc: &FieldDescription,
) -> std::result::Result<FieldInfo, abos_jni::errors::Error> {
	let name: String = field_desc.name(env)?.to_string();
	let documentation: String = field_desc.documentation(env)?.to_string();
	let type_name: String = field_desc.type_name(env)?.to_string();
	let array_dimensions: usize = usize::try_from(field_desc.array_dimensions(env)?)
		.map_err(|error| abos_jni::errors::Error::ParseFailed(error.to_string()))?;
	let is_primitive: bool = field_desc.is_primitive(env)?;
	let is_static: bool = field_desc.is_static(env)?;
	let is_final: bool = field_desc.is_final(env)?;
	let is_deprecated: bool = field_desc.is_deprecated(env)?;

	Ok(FieldInfo {
		name,
		documentation: Some(documentation),
		rust_name_override: {
			let rust_name_jstring: abos_jni::objects::JString<'_> = field_desc.rust_name(env)?;
			if rust_name_jstring.is_null() {
				None
			} else {
				Some(rust_name_jstring.to_string())
			}
		},
		type_info: TypeInfo {
			name: type_name,
			array_dimensions,
			is_primitive,
		},
		is_static,
		is_final,
		is_deprecated,
	})
}
