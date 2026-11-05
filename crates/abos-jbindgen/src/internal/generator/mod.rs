mod documentation;
pub mod identifiers;
mod members;
pub mod names;
mod overloads;
pub mod signatures;

use crate::{BindgenOptions, ClassInfo, ModuleBinding, Result, TypeMap};
use names::derive_rust_type_name;
use std::{
	collections::{HashMap, HashSet},
	fmt::Write as _,
};

pub(super) struct Context<'a> {
	pub class: &'a ClassInfo,
	pub options: &'a BindgenOptions,
	pub types: &'a TypeMap,
	pub used: HashSet<String>,
	pub primitives: HashSet<String>,
	pub body: String,
	// Rust callable name -> full Java member signature (plus accessor role for setters).
	pub member_names: HashMap<String, String>,
}

pub fn generate_with_type_map(class: &ClassInfo, options: &BindgenOptions, types: &TypeMap) -> Result<ModuleBinding> {
	let name: String = derive_rust_type_name(
		&class.class_name,
		options
			.rust_type_name
			.as_deref()
			.or(class.rust_name_override.as_deref()),
		options.name_prefix.as_deref(),
	);
	let mut context = Context {
		class,
		options,
		types,
		used: HashSet::new(),
		primitives: HashSet::new(),
		body: String::new(),
		member_names: HashMap::new(),
	};
	members::constructors(&mut context)?;
	members::fields(&mut context)?;
	members::methods(&mut context, false)?;
	if options.generate_native_interfaces {
		members::methods(&mut context, true)?;
	}
	instance_of(&mut context);
	let mut output: String = String::from("bind_java_type! {\n");
	members::documentation(&mut output, class.documentation.as_deref(), "    ");
	let visibility: &str = if options.public_type { "pub " } else { "" };
	let java_name: String = if class.package.is_empty() {
		format!(".{}", class.class_name)
	} else {
		class.class_name.replace('/', ".")
	};
	writeln!(output, "    {visibility}{name} => \"{java_name}\",").expect("writing to String");
	type_mappings(&context, &mut output);
	output.push_str(&context.body);
	output.push_str("}\n");
	Ok(ModuleBinding {
		package: class.package.join("."),
		module_path: class
			.package
			.iter()
			.map(|part| identifiers::rust_identifier(part))
			.collect(),
		use_statements: vec!["use jni::bind_java_type;".to_owned()],
		binding_code: output,
		rust_api_type_name: format!("{name}API"),
		rust_type_name: name,
		java_type_name: class
			.class_name
			.rsplit('/')
			.next()
			.unwrap_or(&class.class_name)
			.to_owned(),
	})
}

fn instance_of(context: &mut Context<'_>) {
	let mut body = String::new();
	for info in &context.class.instance_of {
		if let Some(rust) = context.types.get_rust_type(&info.java_type) {
			context.used.insert(info.java_type.clone());
			if let Some(stem) = &info.stem {
				writeln!(body, "        {stem}: {rust},").expect("writing to String");
			} else {
				writeln!(body, "        {rust},").expect("writing to String");
			}
		}
	}
	if !body.is_empty() {
		write!(context.body, "    is_instance_of = {{\n{body}    }},\n").expect("writing to String");
	}
}

fn type_mappings(context: &Context<'_>, output: &mut String) {
	if context.used.is_empty() && context.primitives.is_empty() {
		return;
	}
	output.push_str("    type_map = {\n");
	let mut mappings: Vec<(&String, &String)> = context
		.types
		.iter()
		.filter(|(java, _)| context.used.contains(*java))
		.collect();
	mappings.sort_by_key(|(java, _)| *java);
	for (java, rust) in mappings {
		writeln!(output, "        {rust} => \"{java}\",").expect("writing to String");
	}
	let mut primitives: Vec<&String> = context.primitives.iter().collect();
	primitives.sort();
	for rust in primitives {
		if let Some(java) = context.options.primitive_type_map.get(rust) {
			writeln!(output, "        unsafe {rust} => {java},").expect("writing to String");
		}
	}
	output.push_str("    },\n");
}
