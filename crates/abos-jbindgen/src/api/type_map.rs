use crate::internal::generator::{
	identifiers::rust_identifier,
	names::{builtin_jni_types, canonical_java_name, derive_rust_type_name},
	symbols::GeneratedSymbols,
};
use crate::{BindgenOptions, ClassInfo, Result};
use std::collections::HashMap;
/// Maps Java binary names to Rust wrapper type paths.
///
/// Names are stored with dot-separated packages and no leading dot. Inputs also accept
/// slash-separated internal names and the binding macro's `.ClassName` default-package spelling.
#[derive(Debug, Clone)]
pub struct TypeMap {
	/// Maps Java fully-qualified class name (e.g., "android.os.Bundle") to Rust type name
	pub(crate) map: HashMap<String, String>,
}

impl TypeMap {
	/// Create a new `TypeMap` from a collection of `ClassInfo`
	pub(crate) fn from_classes<'a>(
		classes: impl IntoIterator<Item = &'a ClassInfo>,
		options: &BindgenOptions,
	) -> Result<Self> {
		let mut map: HashMap<String, String> = HashMap::new();
		let mut symbols: GeneratedSymbols = GeneratedSymbols::default();

		for class_info in classes {
			// Build the Java type name using class_name which has the full binary name with $ for inner classes
			let java_type: String = canonical_java_name(&class_info.class_name).into_owned();

			// Get the simple Rust type name
			let rust_name: String = derive_rust_type_name(
				&class_info.class_name,
				options
					.rust_type_name
					.as_deref()
					.or(class_info.rust_name_override.as_deref()),
				options.name_prefix.as_deref(),
			);

			// Build fully-qualified Rust path
			// e.g., "crate::bindings::sdk::com::example::MyClass"
			let mut rust_path: String = options.root_path.clone();
			for segment in &class_info.package {
				rust_path.push_str("::");
				rust_path.push_str(&rust_identifier(segment));
			}
			rust_path.push_str("::");
			rust_path.push_str(&rust_name);

			symbols.register_class(class_info, options, &rust_path)?;
			map.insert(java_type, rust_path);
		}

		Ok(Self { map })
	}

	/// Add `(Rust type path, Java binary name)` mappings.
	/// Silently ignores attempts to remap core types (java.lang.String, java.lang.Object, etc.)
	pub fn merge(&mut self, extra_mappings: Vec<(String, String)>) {
		for (rust_type, java_type) in extra_mappings {
			let java_type: String = canonical_java_name(&java_type).into_owned();
			// Check if this is a core builtin type that cannot be remapped
			let is_core: bool = builtin_jni_types()
				.iter()
				.any(|b| b.java_name == java_type && b.is_core);

			if is_core {
				log::warn!("Ignoring attempt to remap core type '{java_type}' to '{rust_type}'");
				continue;
			}

			self.map.insert(java_type, rust_type);
		}
	}

	/// Look up the Rust type name for a Java class
	#[must_use]
	pub fn get_rust_type(&self, java_type: &str) -> Option<&str> {
		self.map
			.get(canonical_java_name(java_type).as_ref())
			.map(std::string::String::as_str)
	}

	/// Get all mappings with canonical Java binary names (no leading dot for the default package).
	pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
		self.map.iter()
	}

	/// Check if the map is empty
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.map.is_empty()
	}

	/// Create a new empty `TypeMap`
	#[must_use]
	pub fn new() -> Self {
		Self { map: HashMap::new() }
	}

	/// Insert a single type mapping
	pub fn insert(&mut self, java_type: String, rust_type: String) {
		let java_type: String = if java_type.starts_with('.') || java_type.contains('/') {
			canonical_java_name(&java_type).into_owned()
		} else {
			java_type
		};
		// Check if this is a core builtin type that cannot be remapped
		let is_core: bool = builtin_jni_types()
			.iter()
			.any(|b| b.java_name == java_type && b.is_core);

		if !is_core {
			self.map.insert(java_type, rust_type);
		}
	}

	/// Get the number of mappings
	#[must_use]
	pub fn len(&self) -> usize {
		self.map.len()
	}
}

impl Default for TypeMap {
	/// Return the default configuration.
	fn default() -> Self {
		Self::new()
	}
}
