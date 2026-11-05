use crate::{ModuleBinding, ModuleEvent, Result, TypeMap, internal::bindings};
use std::{collections::BTreeMap, fmt, fs, path::Path};

/// Generated code and metadata for a single Java binding.
#[derive(Debug, Clone)]
pub struct BindingInfo {
	/// Rust wrapper type name.
	pub rust_type_name: String,
	/// Rust cached-API type name.
	pub rust_api_type_name: String,
	/// Generated macro invocation.
	pub code: String,
	/// Java binary name without its package.
	pub java_type_name: String,
	/// Dot-separated Java package.
	pub package: String,
}

/// A node in the generated Rust module hierarchy.
#[derive(Debug, Clone)]
pub struct Module {
	/// Rust module name; empty for the default Java package.
	pub name: String,
	pub(crate) children: BTreeMap<String, Self>,
	/// Bindings defined directly in this module.
	pub bindings: Vec<BindingInfo>,
	/// Imports needed by this module.
	pub use_statements: Vec<String>,
}

/// Generated bindings with deterministic module ordering.
#[derive(Debug, Clone)]
pub struct Bindings {
	pub(crate) root_modules: BTreeMap<String, Module>,
	pub(crate) generate_jni_init: bool,
	pub(crate) input_type_mappings: Vec<(String, String)>,
	pub(crate) root_path: String,
}

impl Bindings {
	/// Assemble a module hierarchy from generated class bindings.
	pub(crate) fn new(
		modules: Vec<ModuleBinding>,
		generate_jni_init: bool,
		input_type_mappings: Vec<(String, String)>,
		root_path: String,
	) -> Self {
		bindings::assemble(modules, generate_jni_init, input_type_mappings, root_path)
	}

	/// Write all bindings into a single Rust source file.
	///
	/// # Errors
	/// Returns filesystem errors when writing the file.
	pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
		fs::write(path, self.to_string())?;
		Ok(())
	}

	/// Write a module directory tree rooted at `base_path/mod.rs`.
	///
	/// # Errors
	/// Returns filesystem errors when creating directories or writing files.
	pub fn write_to_files(&self, base_path: impl AsRef<Path>) -> Result<()> {
		bindings::write_tree(self, base_path.as_ref())
	}

	/// Visit each module before and after its children, in sorted order.
	pub fn walk<F: FnMut(&Module, ModuleEvent)>(&self, callback: &mut F) {
		for module in self.root_modules.values() {
			bindings::walk(module, callback);
		}
	}

	/// Return the number of generated class bindings.
	#[must_use]
	pub fn len(&self) -> usize {
		let mut count = 0;
		self.walk(&mut |module, event| {
			if event == ModuleEvent::BeginModule {
				count += module.bindings.len();
			}
		});
		count
	}

	/// Return whether no classes were generated.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Combine configured and generated type mappings, optionally replacing the Rust root path.
	#[must_use]
	pub fn type_map(&self, pub_root_path: Option<&str>) -> TypeMap {
		bindings::type_map(self, pub_root_path)
	}

	/// Write sorted type mappings using the configured Rust root path.
	///
	/// # Errors
	/// Returns filesystem errors when writing the file.
	pub fn write_type_map(&self, path: impl AsRef<Path>) -> Result<()> {
		bindings::write_type_map(&self.type_map(None), path.as_ref())
	}

	/// Write sorted type mappings using a replacement Rust root path.
	///
	/// # Errors
	/// Returns filesystem errors when writing the file.
	pub fn write_pub_type_map(&self, path: impl AsRef<Path>, pub_root_path: &str) -> Result<()> {
		bindings::write_type_map(&self.type_map(Some(pub_root_path)), path.as_ref())
	}
}

impl fmt::Display for Bindings {
	/// Render bindings and optional initialization functions as one Rust source file.
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&bindings::render(self))
	}
}
