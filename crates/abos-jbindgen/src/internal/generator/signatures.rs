use super::{
	identifiers::unique_identifier,
	names::{java_name_to_rust, resolve_type_with_deps},
};
use crate::{BindgenOptions, Error, MethodInfo, Result, TypeMap};
use std::collections::HashSet;
use std::fmt::Write as _;
/// Sorting key in `(type1,type2,...)return_type` form, using Java names rather than JNI descriptors.
pub fn generate_jni_signature_for_sorting(method: &MethodInfo) -> String {
	let params: Vec<String> = method
		.signature
		.arguments
		.iter()
		.map(|arg| format_type_for_sorting(&arg.type_info))
		.collect();

	format!(
		"({}){}",
		params.join(","),
		format_type_for_sorting(&method.signature.return_type)
	)
}

pub fn format_type_for_sorting(type_info: &crate::api::types::TypeInfo) -> String {
	let base: String = type_info.name.clone();
	if type_info.array_dimensions > 0 {
		format!("{}{}", base, "[]".repeat(type_info.array_dimensions))
	} else {
		base
	}
}

pub fn type_to_dex_signature(type_info: &crate::api::types::TypeInfo) -> String {
	let mut sig = String::new();

	for _ in 0..type_info.array_dimensions {
		sig.push('[');
	}

	if type_info.is_primitive {
		let code: &str = match type_info.name.as_str() {
			"boolean" => "Z",
			"byte" => "B",
			"char" => "C",
			"short" => "S",
			"int" => "I",
			"long" => "J",
			"float" => "F",
			"double" => "D",
			"void" => "V",
			_ => panic!("Unknown primitive type: {}", type_info.name),
		};
		sig.push_str(code);
	} else {
		sig.push('L');
		sig.push_str(&type_info.name.replace('.', "/"));
		sig.push(';');
	}

	sig
}

/// DEX member key: `Lclass/path;->method(params)return_type`.
pub fn generate_dex_signature(class_name: &str, method: &MethodInfo) -> String {
	let mut sig = String::new();

	sig.push('L');
	sig.push_str(&class_name.replace('.', "/"));
	sig.push_str(";->");

	sig.push_str(&method.name);

	sig.push('(');
	for arg in &method.signature.arguments {
		sig.push_str(&type_to_dex_signature(&arg.type_info));
	}
	sig.push(')');

	sig.push_str(&type_to_dex_signature(&method.signature.return_type));

	sig
}

/// DEX member key: `Lclass/path;->field:type`.
pub fn generate_field_dex_signature(class_name: &str, field: &crate::api::types::FieldInfo) -> String {
	let mut sig = String::new();

	sig.push('L');
	sig.push_str(&class_name.replace('.', "/"));
	sig.push_str(";->");

	sig.push_str(&field.name);

	sig.push(':');
	sig.push_str(&type_to_dex_signature(&field.type_info));

	sig
}

/// Render a method signature while tracking referenced wrapper and primitive types.
pub fn generate_method_signature_with_deps(
	method: &MethodInfo,
	type_map: &TypeMap,
	options: &BindgenOptions,
	used_types: &mut HashSet<String>,
	used_primitive_types: &mut HashSet<String>,
) -> Result<String> {
	let mut sig: String = String::from("(");
	// These names are parameters or locals emitted by bind_java_type! itself.
	let mut names: HashSet<String> = ["env", "api", "jni_args", "class", "this", "unowned_env"]
		.into_iter()
		.map(str::to_owned)
		.collect();

	for (i, arg) in method.signature.arguments.iter().enumerate() {
		if i > 0 {
			sig.push_str(", ");
		}

		let arg_name: String = arg
			.name
			.as_deref()
			.map_or_else(|| format!("arg{i}"), |name| java_name_to_rust(name).0);
		let arg_name: String = unique_identifier(&arg_name, &mut names);
		write!(sig, "{arg_name}: ").expect("writing to String");

		if let Some(rust_prim) = &arg.rust_primitive {
			validate_primitive(rust_prim, &arg.type_info, options)?;
			sig.push_str(rust_prim);
			used_primitive_types.insert(rust_prim.clone());
		} else {
			sig.push_str(&resolve_type_with_deps(&arg.type_info, type_map, used_types));
		}
	}

	sig.push(')');

	if method.signature.return_type.name != "void" {
		sig.push_str(" -> ");
		sig.push_str(&resolve_type_with_deps(
			&method.signature.return_type,
			type_map,
			used_types,
		));
	}

	Ok(sig)
}

fn validate_primitive(rust: &str, ty: &crate::TypeInfo, options: &BindgenOptions) -> Result<()> {
	let java: &String = options.primitive_type_map.get(rust).ok_or_else(|| {
		Error::CodeGen(format!(
			"No unsafe primitive type mapping found for @RustPrimitive(\"{rust}\")"
		))
	})?;
	if !ty.is_primitive || ty.array_dimensions != 0 || java != &ty.name {
		return Err(Error::CodeGen(format!(
			"Type mismatch for @RustPrimitive(\"{rust}\"): expected {java}, found {}",
			ty.name
		)));
	}
	Ok(())
}
