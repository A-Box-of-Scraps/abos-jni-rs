use super::{
	names::{java_name_to_rust, resolve_type_with_deps},
	overloads::sanitize_rust_keyword,
};
use crate::{BindgenOptions, Error, MethodInfo, Result, TypeMap};
use std::collections::HashSet;
use std::fmt::Write as _;
/// Generate a JNI-style signature string for a method for sorting purposes
/// Format: (`arg1_type,arg2_type,...)return_type`
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

/// Format a type for sorting purposes
pub fn format_type_for_sorting(type_info: &crate::api::types::TypeInfo) -> String {
	let base: String = type_info.name.clone();
	if type_info.array_dimensions > 0 {
		format!("{}{}", base, "[]".repeat(type_info.array_dimensions))
	} else {
		base
	}
}

/// Convert a `TypeInfo` to DEX signature format
pub fn type_to_dex_signature(type_info: &crate::api::types::TypeInfo) -> String {
	let mut sig = String::new();

	// Add array dimensions
	for _ in 0..type_info.array_dimensions {
		sig.push('[');
	}

	// Add the base type
	if type_info.is_primitive {
		// Primitive types use single character codes
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
		// Object types use L<class_path>;
		sig.push('L');
		sig.push_str(&type_info.name.replace('.', "/"));
		sig.push(';');
	}

	sig
}

/// Generate a DEX signature for a method
/// Format: L<`class_path`>;-><`method_name`>(<params>)<`return_type`>
pub fn generate_dex_signature(class_name: &str, method: &MethodInfo) -> String {
	let mut sig = String::new();

	// Add class name in DEX format
	sig.push('L');
	sig.push_str(&class_name.replace('.', "/"));
	sig.push_str(";->");

	// Add method name
	sig.push_str(&method.name);

	// Add parameter signatures
	sig.push('(');
	for arg in &method.signature.arguments {
		sig.push_str(&type_to_dex_signature(&arg.type_info));
	}
	sig.push(')');

	// Add return type signature
	sig.push_str(&type_to_dex_signature(&method.signature.return_type));

	sig
}

/// Generate a DEX signature for a field
/// Format: L<`class_path`>;-><`field_name>`:<type>
pub fn generate_field_dex_signature(class_name: &str, field: &crate::api::types::FieldInfo) -> String {
	let mut sig = String::new();

	// Add class name in DEX format
	sig.push('L');
	sig.push_str(&class_name.replace('.', "/"));
	sig.push_str(";->");

	// Add field name
	sig.push_str(&field.name);

	// Add field type
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

	for (i, arg) in method.signature.arguments.iter().enumerate() {
		if i > 0 {
			sig.push_str(", ");
		}

		let arg_name: String = arg.name.as_deref().map_or_else(
			|| format!("arg{i}"),
			|name| sanitize_rust_keyword(&java_name_to_rust(name).0),
		);
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

	// Add return type if not void
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
