use std::collections::HashMap;

/// Event emitted during module traversal
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleEvent {
	/// Beginning of a module
	BeginModule,
	/// End of a module
	EndModule,
}

/// A binding for a single Java type within a module
#[derive(Debug, Clone)]
pub struct ModuleBinding {
	/// The Java package name (e.g., "com.example.stuff")
	pub package: String,
	/// The Rust module path (e.g., `com::example::stuff`)
	pub module_path: Vec<String>,
	/// Fully-qualified use statements (canonical paths, one item per use)
	pub use_statements: Vec<String>,
	/// The generated binding code for a single Java type
	pub binding_code: String,
	/// The Rust type name (e.g., "`MyClass`")
	pub rust_type_name: String,
	/// The Rust API type name (e.g., "`MyClassAPI`")
	pub rust_api_type_name: String,
	/// The Java type name (e.g., "`MyClass`" or "MyClass$Inner")
	pub java_type_name: String,
}

/// Options for binding generation
#[derive(Debug, Clone)]
pub struct BindgenOptions {
	/// Whether to make the generated type public
	pub public_type: bool,
	/// Name override for the Rust type (if None, uses the Java simple class name)
	pub rust_type_name: Option<String>,
	/// Prefix to add to all generated Rust type names
	pub name_prefix: Option<String>,
	/// Whether to generate native method interfaces (default: true)
	pub generate_native_interfaces: bool,
	/// Whether to generate `jni_init` methods for modules (default: true)
	pub generate_jni_init: bool,
	/// Root module path for generated bindings (e.g., "`crate::bindings::sdk`")
	pub root_path: String,
	/// DEX signatures of methods/fields to skip during generation
	pub skip_signatures: Vec<String>,
	/// Map of DEX signatures to override Rust names
	pub name_overrides: HashMap<String, String>,
	/// Unsafe primitive type mappings (`RustType` => javaPrimitive)
	pub primitive_type_map: HashMap<String, String>,
}

impl Default for BindgenOptions {
	/// Return the default configuration.
	fn default() -> Self {
		Self {
			public_type: true,
			rust_type_name: None,
			name_prefix: None,
			generate_native_interfaces: true,
			generate_jni_init: true,
			root_path: String::from("crate"),
			skip_signatures: Vec::new(),
			name_overrides: HashMap::new(),
			primitive_type_map: HashMap::new(),
		}
	}
}
