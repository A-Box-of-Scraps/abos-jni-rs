use crate::{ClassInfo, FieldInfo, MethodInfo};
use std::collections::{HashMap, HashSet};
pub fn intersect_classes(
	jar_classes: HashMap<String, ClassInfo>,
	stub_classes: &HashMap<String, ClassInfo>,
) -> Vec<ClassInfo> {
	let mut result: Vec<crate::ClassInfo> = Vec::new();

	for (class_name, jar_class) in jar_classes {
		if let Some(stub_class) = stub_classes.get(&class_name) {
			let intersected: crate::ClassInfo = intersect_class_members(&jar_class, stub_class);
			result.push(intersected);
		}
	}

	result
}

/// Intersect members of two `ClassInfo` instances
pub fn intersect_class_members(jar_class: &ClassInfo, stub_class: &ClassInfo) -> ClassInfo {
	let jar_constructor_sigs: HashSet<String> = jar_class.constructors.iter().map(method_signature_key).collect();

	let jar_method_sigs: HashSet<String> = jar_class.methods.iter().map(method_signature_key).collect();

	let jar_native_sigs: HashSet<String> = jar_class.native_methods.iter().map(method_signature_key).collect();

	let jar_field_sigs: HashSet<String> = jar_class.fields.iter().map(field_signature_key).collect();

	// Keep stub metadata to preserve documentation and deprecation information.
	let constructors: Vec<crate::MethodInfo> = stub_class
		.constructors
		.iter()
		.filter(|m| jar_constructor_sigs.contains(&method_signature_key(m)))
		.cloned()
		.collect();

	let methods: Vec<crate::MethodInfo> = stub_class
		.methods
		.iter()
		.filter(|m| jar_method_sigs.contains(&method_signature_key(m)))
		.cloned()
		.collect();

	let native_methods: Vec<crate::MethodInfo> = stub_class
		.native_methods
		.iter()
		.filter(|m| jar_native_sigs.contains(&method_signature_key(m)))
		.cloned()
		.collect();

	let fields: Vec<crate::FieldInfo> = stub_class
		.fields
		.iter()
		.filter(|f| jar_field_sigs.contains(&field_signature_key(f)))
		.cloned()
		.collect();

	ClassInfo {
		class_name: stub_class.class_name.clone(),
		package: stub_class.package.clone(),
		simple_name: stub_class.simple_name.clone(),
		documentation: stub_class.documentation.clone(),
		rust_name_override: stub_class.rust_name_override.clone(),
		constructors,
		methods,
		fields,
		native_methods,
		instance_of: stub_class.instance_of.clone(),
	}
}

/// Create a signature key for a method (name + parameter types + return type + static flag)
pub fn method_signature_key(method: &MethodInfo) -> String {
	let params: Vec<String> = method
		.signature
		.arguments
		.iter()
		.map(|arg| format_type_signature(&arg.type_info))
		.collect();

	format!(
		"{}({}){}:{}",
		method.name,
		params.join(","),
		format_type_signature(&method.signature.return_type),
		method.is_static
	)
}

/// Create a signature key for a field (name + type + static flag)
pub fn field_signature_key(field: &FieldInfo) -> String {
	format!(
		"{}:{}:{}",
		field.name,
		format_type_signature(&field.type_info),
		field.is_static
	)
}

/// Format a type signature for comparison
pub fn format_type_signature(type_info: &crate::api::types::TypeInfo) -> String {
	if type_info.array_dimensions > 0 {
		format!("{}{}", type_info.name, "[]".repeat(type_info.array_dimensions))
	} else {
		type_info.name.clone()
	}
}
