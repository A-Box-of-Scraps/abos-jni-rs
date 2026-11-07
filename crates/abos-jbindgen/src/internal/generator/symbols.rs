//! Names emitted alongside Java members by the binding macro and module renderer.
use super::{identifiers::rust_identifier, signatures::generate_dex_signature};
use crate::{BindgenOptions, ClassInfo, Error, Result, TypeMap};
use std::collections::HashMap;

pub(super) fn member_names(class: &ClassInfo, types: &TypeMap) -> HashMap<String, String> {
	// Keep the inherent names in sync with bind_java_type::generate_base_methods.
	// as_ref/as_raw/is_null are also used by generated calls; shadowing these trait
	// methods with Java methods would break the generated implementation itself.
	let mut names: HashMap<String, String> = [
		"from_raw",
		"into_raw",
		"null",
		"cast_local",
		"as_ref",
		"as_raw",
		"is_null",
	]
	.into_iter()
	.map(|name| (name.to_owned(), format!("generated reference method {name}")))
	.collect();
	for parent in &class.instance_of {
		if let Some(stem) = &parent.stem
			&& types.get_rust_type(&parent.java_type).is_some()
		{
			let name: String = format!("as_{stem}");
			names.insert(name.clone(), format!("generated superclass accessor {name}"));
		}
	}
	names
}

/// Track type and value namespaces separately: modules and functions can share names,
/// but tuple-struct wrappers occupy both namespaces.
#[derive(Default)]
pub struct GeneratedSymbols {
	types: HashMap<String, String>,
	values: HashMap<String, String>,
}

impl GeneratedSymbols {
	pub fn register_class(&mut self, class: &ClassInfo, options: &BindgenOptions, rust_path: &str) -> Result<()> {
		let mut scope: String = options.root_path.clone();
		self.initializer(&scope, options.generate_jni_init)?;
		for segment in &class.package {
			scope.push_str("::");
			scope.push_str(&rust_identifier(segment));
			reserve(
				&mut self.types,
				&scope,
				format!("Java package module {scope:?}"),
				"type",
			)?;
			self.initializer(&scope, options.generate_jni_init)?;
		}
		let owner: String = format!("Java class {:?}", class.class_name);
		reserve(&mut self.types, rust_path, owner.clone(), "type")?;
		reserve(&mut self.values, rust_path, owner, "value")?;
		reserve(
			&mut self.types,
			&format!("{rust_path}API"),
			format!("generated API type for {:?}", class.class_name),
			"type",
		)?;
		let has_native_interface: bool = options.generate_native_interfaces
			&& class.native_methods.iter().any(|method| {
				!options
					.skip_signatures
					.contains(&generate_dex_signature(&class.class_name, method))
			});
		if has_native_interface {
			reserve(
				&mut self.types,
				&format!("{rust_path}NativeInterface"),
				format!("generated native interface for {:?}", class.class_name),
				"type",
			)?;
		}
		Ok(())
	}

	fn initializer(&mut self, scope: &str, enabled: bool) -> Result<()> {
		if enabled {
			reserve(
				&mut self.values,
				&format!("{scope}::jni_init"),
				"generated module initializer".to_owned(),
				"value",
			)?;
		}
		Ok(())
	}
}

fn reserve(names: &mut HashMap<String, String>, path: &str, owner: String, namespace: &str) -> Result<()> {
	if let Some(previous) = names.get(path)
		&& previous != &owner
	{
		return Err(Error::CodeGen(format!(
			"{previous} and {owner} map to the same Rust {namespace} {path:?}; use distinct Rust type names, disable the conflicting generated helper, or generate them separately"
		)));
	}
	names.insert(path.to_owned(), owner);
	Ok(())
}
