use super::{
	Context,
	documentation::format_javadoc_as_rustdoc,
	names::{java_name_to_rust, resolve_type_with_deps},
	overloads::{generate_unique_overload_names, group_methods_by_name, is_snake_case, resolve_name_collision},
	signatures::{
		generate_dex_signature, generate_field_dex_signature, generate_jni_signature_for_sorting,
		generate_method_signature_with_deps,
	},
};
use crate::{FieldInfo, MethodInfo, Result};
use std::{collections::HashSet, fmt::Write as _};

pub(super) fn documentation(output: &mut String, docs: Option<&str>, indent: &str) {
	if let Some(docs) = docs.filter(|docs| !docs.is_empty()) {
		for line in format_javadoc_as_rustdoc(docs).lines() {
			writeln!(output, "{indent}{line}").expect("writing to String");
		}
	}
}

fn attributes(output: &mut String, docs: Option<&str>, deprecated: bool, name: &str) {
	documentation(output, docs, "        ");
	if deprecated {
		output.push_str("        #[deprecated]\n");
	}
	if !is_snake_case(name) {
		output.push_str("        #[allow(non_snake_case, reason = \"Java API name\")]\n");
	}
}

pub(super) fn constructors(context: &mut Context<'_>) -> Result<()> {
	let mut methods: Vec<&MethodInfo> = context
		.class
		.constructors
		.iter()
		.filter(|method| {
			!context
				.options
				.skip_signatures
				.contains(&generate_dex_signature(&context.class.class_name, method))
		})
		.collect();
	if methods.is_empty() {
		return Ok(());
	}
	methods.sort_by_key(|method| {
		(
			method.signature.arguments.len(),
			generate_jni_signature_for_sorting(method),
		)
	});
	let names: Vec<String> = generate_unique_overload_names("new", &methods, &mut HashSet::new());
	context.body.push_str("    constructors {\n");
	for (method, generated) in methods.iter().zip(&names) {
		let dex: String = generate_dex_signature(&context.class.class_name, method);
		let name: &str = method
			.rust_name_override
			.as_deref()
			.or_else(|| context.options.name_overrides.get(&dex).map(String::as_str))
			.unwrap_or(generated);
		let sig: String = generate_method_signature_with_deps(
			method,
			context.types,
			context.options,
			&mut context.used,
			&mut context.primitives,
		)?;
		attributes(
			&mut context.body,
			method.documentation.as_deref(),
			method.is_deprecated,
			name,
		);
		writeln!(context.body, "        fn {name}{sig},").expect("writing to String");
	}
	context.body.push_str("    },\n");
	Ok(())
}

pub(super) fn fields(context: &mut Context<'_>) {
	let fields: Vec<&FieldInfo> = context
		.class
		.fields
		.iter()
		.filter(|field| {
			!context
				.options
				.skip_signatures
				.contains(&generate_field_dex_signature(&context.class.class_name, field))
		})
		.collect();
	if fields.is_empty() {
		return;
	}
	context.body.push_str("    fields {\n");
	for field in fields {
		let dex: String = generate_field_dex_signature(&context.class.class_name, field);
		let (name, reversible) = member_name(context, &field.name, &dex, field.rust_name_override.as_deref(), None);
		context.member_names.remove(&(name.clone(), field.name.clone()));
		context
			.member_names
			.insert((name.clone(), format!("field:{}", field.name)));
		let setter: Option<String> = if field.is_final {
			None
		} else {
			Some(
				resolve_name_collision(
					format!("set_{name}"),
					&format!("field-setter:{}", field.name),
					&mut context.member_names,
				)
				.0,
			)
		};
		let ty: String = resolve_type_with_deps(&field.type_info, context.types, &mut context.used);
		attributes(
			&mut context.body,
			field.documentation.as_deref(),
			field.is_deprecated,
			&name,
		);
		let modifier: &str = if field.is_static { "static " } else { "" };
		if reversible && setter.as_deref() == Some(format!("set_{name}").as_str()) {
			writeln!(context.body, "        {modifier}{name}: {ty},").expect("writing to String");
		} else {
			writeln!(context.body, "        {modifier}{name} {{").expect("writing to String");
			if !reversible {
				writeln!(context.body, "            name = \"{}\",", field.name).expect("writing to String");
			}
			writeln!(context.body, "            sig = {ty},").expect("writing to String");
			writeln!(context.body, "            get = {name},").expect("writing to String");
			if let Some(setter) = setter {
				writeln!(context.body, "            set = {setter},").expect("writing to String");
			}
			context.body.push_str("        },\n");
		}
	}
	context.body.push_str("    },\n");
}

fn member_name(
	context: &mut Context<'_>,
	java: &str,
	dex: &str,
	annotation: Option<&str>,
	overload: Option<&str>,
) -> (String, bool) {
	let override_name: Option<&str> = context
		.options
		.name_overrides
		.get(dex)
		.map(String::as_str)
		.or(annotation)
		.or(overload);
	let (name, reversible) = override_name.map_or_else(|| java_name_to_rust(java), |name| (name.to_owned(), false));
	let (resolved, collision, previous) = resolve_name_collision(name, java, &mut context.member_names);
	if collision {
		log::warn!("Java member {java} conflicts with {previous:?}; using {resolved}");
	}
	(resolved, reversible && !collision)
}

pub(super) fn methods(context: &mut Context<'_>, native: bool) -> Result<()> {
	let mut methods: Vec<MethodInfo> = if native {
		context.class.native_methods.clone()
	} else {
		context.class.methods.clone()
	};
	if !native && !context.options.generate_native_interfaces {
		methods.extend(
			context
				.class
				.native_methods
				.iter()
				.filter(|method| method.is_public)
				.cloned(),
		);
	}
	methods.retain(|method| {
		!context
			.options
			.skip_signatures
			.contains(&generate_dex_signature(&context.class.class_name, method))
	});
	if methods.is_empty() {
		return Ok(());
	}
	let block: &str = if native { "native_methods" } else { "methods" };
	writeln!(context.body, "    {block} {{").expect("writing to String");
	for (java, group) in group_methods_by_name(&methods) {
		let base: String = java_name_to_rust(java).0;
		let names: Vec<String> = generate_unique_overload_names(&base, &group, &mut HashSet::new());
		for (method, generated) in group.iter().zip(&names) {
			let dex: String = generate_dex_signature(&context.class.class_name, method);
			let overload: Option<&str> = (group.len() > 1).then_some(generated.as_str());
			let (name, reversible) = member_name(context, java, &dex, method.rust_name_override.as_deref(), overload);
			method_binding(context, method, &name, reversible, native)?;
		}
	}
	context.body.push_str("    },\n");
	Ok(())
}

fn method_binding(
	context: &mut Context<'_>,
	method: &MethodInfo,
	name: &str,
	reversible: bool,
	native: bool,
) -> Result<()> {
	let sig: String = generate_method_signature_with_deps(
		method,
		context.types,
		context.options,
		&mut context.used,
		&mut context.primitives,
	)?;
	attributes(
		&mut context.body,
		method.documentation.as_deref(),
		method.is_deprecated,
		name,
	);
	let modifier: &str = if method.is_static { "static " } else { "" };
	let visibility: &str = if native && method.is_public { "pub " } else { "" };
	if reversible {
		writeln!(context.body, "        {visibility}{modifier}fn {name}{sig},").expect("writing to String");
	} else {
		writeln!(
			context.body,
			"        {visibility}{modifier}fn {name} {{\n            name = \"{}\",\n            sig = {sig},\n        }},",
			method.name
		)
		.expect("writing to String");
	}
	Ok(())
}
