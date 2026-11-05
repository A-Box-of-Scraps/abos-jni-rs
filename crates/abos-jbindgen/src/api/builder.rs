use crate::internal::{
	android,
	inputs::{
		generate_bindings_from_bytes, generate_bindings_from_jar, generate_bindings_from_sources,
		generate_bindings_with_type_map,
	},
};
use crate::{BindgenOptions, Bindings, Error, HiddenApiFilter, Result};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};
/// Input source for generating bindings.
#[derive(Debug, Clone)]
pub enum InputSource {
	/// A single .class file
	ClassFile(PathBuf),
	/// Raw bytes of a .class file
	ClassBytes(Vec<u8>),
	/// A JAR file with optional class patterns for filtering
	JarFile { path: PathBuf, patterns: Vec<String> },
	/// Java source files
	Sources {
		source_paths: Vec<PathBuf>,
		classpath: Vec<PathBuf>,
		patterns: Vec<String>,
	},
	/// Android SDK
	AndroidSdk {
		api_level: u32,
		patterns: Vec<String>,
		hiddenapi_flags_path: Option<PathBuf>,
	},
}

/// Builder for configuring and generating Java bindings.
#[must_use]
#[derive(Debug, Clone)]
pub struct Builder {
	pub(crate) input: Option<InputSource>,
	public_type: bool,
	rust_type_name: Option<String>,
	name_prefix: Option<String>,
	type_map: Vec<(String, String)>,
	primitive_type_map: Vec<(String, String)>,
	generate_native_interfaces: bool,
	generate_jni_init: bool,
	hidden_api_filter: HiddenApiFilter,
	root_path: String,
	skip_signatures: Vec<String>,
	name_overrides: HashMap<String, String>,
}

impl Default for Builder {
	/// Return the default configuration.
	fn default() -> Self {
		Self::new()
	}
}

impl Builder {
	/// Create a new builder with default settings.
	pub fn new() -> Self {
		Self {
			input: None,
			public_type: true,
			rust_type_name: None,
			name_prefix: None,
			type_map: Vec::new(),
			primitive_type_map: Vec::new(),
			generate_native_interfaces: true,
			generate_jni_init: true,
			hidden_api_filter: HiddenApiFilter::default(),
			root_path: String::from("crate"),
			skip_signatures: Vec::new(),
			name_overrides: HashMap::new(),
		}
	}

	/// Set the input to a .class file.
	pub fn input_class<P: AsRef<Path>>(mut self, path: P) -> Self {
		self.input = Some(InputSource::ClassFile(path.as_ref().to_path_buf()));
		self
	}

	/// Set the input to raw .class file bytes.
	pub fn input_class_bytes(mut self, bytes: Vec<u8>) -> Self {
		self.input = Some(InputSource::ClassBytes(bytes));
		self
	}

	/// Set the input to a JAR file.
	///
	/// Generates bindings from base class entries, in archive-name order. Ignores
	/// `module-info.class`, `package-info.class`, and everything under `META-INF/`,
	/// including multi-release variants. Selection never depends on the host JVM.
	/// Version-only classes and members are not included, even with explicit patterns.
	/// The JVM still chooses its runtime variant, which must retain the generated base members.
	pub fn input_jar<P: AsRef<Path>>(mut self, path: P) -> Self {
		self.input = Some(InputSource::JarFile {
			path: path.as_ref().to_path_buf(),
			patterns: Vec::new(),
		});
		self
	}

	/// Set the input to Java source files.
	pub fn input_sources(mut self, source_paths: Vec<PathBuf>, classpath: Vec<PathBuf>, patterns: Vec<String>) -> Self {
		self.input = Some(InputSource::Sources {
			source_paths,
			classpath,
			patterns,
		});
		self
	}

	/// Set the input to Android SDK APIs.
	///
	/// Generation fails on the first selected-class parsing or code-generation error;
	/// it never returns partial bindings after silently dropping a failing class.
	/// Class failures preserve the class/archive context and underlying error.
	pub fn input_android_sdk(mut self, api_level: u32, patterns: Vec<String>) -> Self {
		self.input = Some(InputSource::AndroidSdk {
			api_level,
			patterns,
			hiddenapi_flags_path: None,
		});
		self
	}

	/// Set the path to hiddenapi-flags.csv for filtering Android SDK APIs.
	pub fn hiddenapi_flags<P: AsRef<Path>>(mut self, path: P) -> Self {
		if let Some(InputSource::AndroidSdk {
			hiddenapi_flags_path, ..
		}) = &mut self.input
		{
			*hiddenapi_flags_path = Some(path.as_ref().to_path_buf());
		}
		self
	}

	/// Allow "unsupported" APIs when filtering with hiddenapi-flags.csv.
	/// This affects hidden-API eligibility only; parsing and generation errors still fail.
	pub const fn allow_unsupported(mut self, allow: bool) -> Self {
		self.hidden_api_filter.allow_unsupported = allow;
		self
	}

	/// Set maximum target level for conditional API support when filtering with hiddenapi-flags.csv.
	pub fn max_target<S: Into<String>>(mut self, target: S) -> Self {
		self.hidden_api_filter.max_target = Some(target.into());
		self
	}

	/// Make the generated type public (default: true).
	pub const fn public_type(mut self, public: bool) -> Self {
		self.public_type = public;
		self
	}

	/// Make the generated type private.
	pub const fn private_type(mut self, private: bool) -> Self {
		self.public_type = !private;
		self
	}

	/// Override the generated Rust type name.
	pub fn rust_type_name<S: Into<String>>(mut self, name: S) -> Self {
		self.rust_type_name = Some(name.into());
		self
	}

	/// Add a prefix to all generated Rust type names.
	pub fn name_prefix<S: Into<String>>(mut self, prefix: S) -> Self {
		self.name_prefix = Some(prefix.into());
		self
	}

	/// Set class patterns for filtering which classes to generate bindings for.
	pub fn patterns<I, S>(mut self, patterns: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		let patterns_vec: Vec<String> = patterns.into_iter().map(std::convert::Into::into).collect();

		if let Some(
			InputSource::JarFile { patterns, .. }
			| InputSource::Sources { patterns, .. }
			| InputSource::AndroidSdk { patterns, .. },
		) = &mut self.input
		{
			*patterns = patterns_vec;
		} else {
			// Ignore for ClassFile and ClassBytes inputs
		}
		self
	}

	/// Add a type mapping from Rust type to Java type.
	pub fn type_mapping<S1: Into<String>, S2: Into<String>>(mut self, rust_type: S1, java_type: S2) -> Self {
		let rust_type_str: String = rust_type.into();
		let java_type_str: String = java_type.into();

		// Check if this is an unsafe primitive mapping
		if let Some(stripped) = rust_type_str.strip_prefix("unsafe ") {
			self.primitive_type_map
				.push((stripped.trim().to_string(), java_type_str));
		} else {
			self.type_map.push((rust_type_str, java_type_str));
		}
		self
	}

	/// Add multiple type mappings.
	pub fn type_mappings(mut self, mappings: Vec<(String, String)>) -> Self {
		for (rust_type, java_type) in mappings {
			// Check if this is an unsafe primitive mapping
			if let Some(stripped) = rust_type.strip_prefix("unsafe ") {
				self.primitive_type_map.push((stripped.trim().to_string(), java_type));
			} else {
				self.type_map.push((rust_type, java_type));
			}
		}
		self
	}

	/// Control whether to generate native method interfaces (default: true).
	pub const fn generate_native_interfaces(mut self, generate: bool) -> Self {
		self.generate_native_interfaces = generate;
		self
	}

	/// Control whether to generate `jni_init` methods for modules (default: true).
	pub const fn generate_jni_init(mut self, generate: bool) -> Self {
		self.generate_jni_init = generate;
		self
	}

	/// Skip methods or fields matching the given DEX signature.
	pub fn skip_signature<S: Into<String>>(mut self, signature: S) -> Self {
		self.skip_signatures.push(signature.into());
		self
	}

	/// Override the Rust name for a constructor, method, or field matching the given DEX signature.
	///
	/// Takes precedence over `@RustName`. Names are normalized to Rust identifiers and
	/// gain trailing underscores when needed to avoid other constructors, methods, or field accessors.
	pub fn name_override<S1: Into<String>, S2: Into<String>>(mut self, signature: S1, name: S2) -> Self {
		self.name_overrides.insert(signature.into(), name.into());
		self
	}

	/// Set the root module path for generated bindings.
	pub fn root_path<S: Into<String>>(mut self, path: S) -> Self {
		self.root_path = path.into();
		self
	}

	/// Generate the bindings.
	///
	/// # Errors
	/// Returns input, parsing, SDK discovery, or code generation errors.
	pub fn generate(self) -> Result<Bindings> {
		let input: InputSource = self
			.input
			.ok_or_else(|| Error::CodeGen("No input source specified".to_string()))?;

		let options = BindgenOptions {
			public_type: self.public_type,
			rust_type_name: self.rust_type_name.clone(),
			name_prefix: self.name_prefix.clone(),
			generate_native_interfaces: self.generate_native_interfaces,
			generate_jni_init: self.generate_jni_init,
			root_path: self.root_path.clone(),
			skip_signatures: self.skip_signatures.clone(),
			name_overrides: self.name_overrides.clone(),
			primitive_type_map: self.primitive_type_map.iter().cloned().collect(),
		};

		let root_path: String = self.root_path.clone();
		let type_map: Vec<(String, String)> = self.type_map.clone();
		let generate_jni_init: bool = self.generate_jni_init;

		match input {
			InputSource::ClassFile(path) => {
				let binding: crate::ModuleBinding = generate_bindings_with_type_map(&path, &options, type_map.clone())?;
				Ok(Bindings::new(vec![binding], generate_jni_init, type_map, root_path))
			}
			InputSource::ClassBytes(bytes) => {
				let binding: crate::ModuleBinding = generate_bindings_from_bytes(&bytes, &options, type_map.clone())?;
				Ok(Bindings::new(vec![binding], generate_jni_init, type_map, root_path))
			}
			InputSource::JarFile { path, patterns } => {
				let bindings: Vec<crate::ModuleBinding> =
					generate_bindings_from_jar(&path, &patterns, &options, type_map.clone())?;
				Ok(Bindings::new(bindings, generate_jni_init, type_map, root_path))
			}
			InputSource::Sources {
				source_paths,
				classpath,
				patterns,
			} => {
				let bindings: Vec<crate::ModuleBinding> =
					generate_bindings_from_sources(&source_paths, &classpath, &patterns, &options, type_map.clone())?;
				Ok(Bindings::new(bindings, generate_jni_init, type_map, root_path))
			}
			InputSource::AndroidSdk {
				api_level,
				patterns,
				hiddenapi_flags_path,
			} => {
				// For Android SDK bindings, native methods are implemented by the platform,
				// so we disable native method interface generation
				let mut android_options: crate::BindgenOptions = options;
				android_options.generate_native_interfaces = false;

				let bindings: Vec<crate::ModuleBinding> = android::generate_android_bindings(
					api_level,
					&patterns,
					hiddenapi_flags_path.as_deref(),
					&self.hidden_api_filter,
					&android_options,
					type_map.clone(),
				)?;
				Ok(Bindings::new(bindings, generate_jni_init, type_map, root_path))
			}
		}
	}
}
