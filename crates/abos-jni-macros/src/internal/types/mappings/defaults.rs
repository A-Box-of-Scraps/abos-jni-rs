use super::{JavaClassName, PrimitiveType, Rc, RustType, RustTypeTarget, TypeMappings, path_to_string_no_spaces};

impl TypeMappings {
	/// Create a new `TypeMappings` with default JNI type mappings
	///
	/// # Arguments
	/// * `jni_crate` - The path to the jni crate (e.g., `jni` or `::jni` or `crate::jni`)
	pub fn new(jni_crate: &syn::Path) -> Self {
		let mut type_mappings = Self {
			alias_to_rust: std::collections::HashMap::new(),
			rust_types: std::collections::HashSet::new(),
			rust_to_java: std::collections::HashMap::new(),
			java_to_rust: std::collections::HashMap::new(),
			core_java: std::collections::HashSet::new(),
			jni_crate: jni_crate.clone(),
		};

		// Convert the jni crate path to a string
		let jni_crate_str = path_to_string_no_spaces(jni_crate);

		type_mappings.add_reference_defaults(&jni_crate_str);

		// Insert Void as a special case
		type_mappings
			.insert_prim_type("void", PrimitiveType::Void, "()", true)
			.expect("Failed to insert 'void' primitive type mapping");

		let jni_sys_prim_types = [
			("jboolean", "sys::jboolean", PrimitiveType::Boolean),
			("jbyte", "sys::jbyte", PrimitiveType::Byte),
			("jchar", "sys::jchar", PrimitiveType::Char),
			("jshort", "sys::jshort", PrimitiveType::Short),
			("jint", "sys::jint", PrimitiveType::Int),
			("jlong", "sys::jlong", PrimitiveType::Long),
			("jfloat", "sys::jfloat", PrimitiveType::Float),
			("jdouble", "sys::jdouble", PrimitiveType::Double),
		];

		for (rust_name, path, prim_type) in jni_sys_prim_types {
			let full_path = format!("{jni_crate_str}::{path}");
			type_mappings
				.insert_prim_type(rust_name, prim_type, &full_path, true)
				.expect("Failed to insert abos_jni::sys primitive type mapping");
		}

		let rust_prim_types = [
			("bool", PrimitiveType::Boolean),
			("i8", PrimitiveType::Byte),
			("i16", PrimitiveType::Short),
			("i32", PrimitiveType::Int),
			("i64", PrimitiveType::Long),
			("f32", PrimitiveType::Float),
			("f64", PrimitiveType::Double),
		];

		for (rust_name, prim_type) in rust_prim_types {
			type_mappings
				.insert_prim_type(rust_name, prim_type, rust_name, true)
				.expect("Failed to insert Rust primitive type mapping");
		}

		let prim_aliases = [
			("boolean", "jboolean"),
			("byte", "jbyte"),
			("char", "jchar"),
			("short", "jshort"),
			("int", "jint"),
			("long", "jlong"),
			("float", "jfloat"),
			("double", "jdouble"),
		];

		for (alias, target) in prim_aliases {
			type_mappings
				.insert_alias(alias, target)
				.expect("Failed to insert primitive type alias");
		}

		type_mappings
	}

	fn add_reference_defaults(&mut self, jni_crate_str: &str) {
		// Helper function to parse a Java class name from a dotted string
		let parse_java_class_dotted = |class_str: &str| -> JavaClassName {
			let parts: Vec<&str> = class_str.split('.').collect();
			assert!(!parts.is_empty(), "Invalid Java class name: {class_str}");

			let class = parts
				.last()
				.expect("built-in class names have at least one component")
				.to_string();
			let package = parts[..parts.len() - 1]
				.iter()
				.map(std::string::ToString::to_string)
				.collect();

			JavaClassName { package, class }
		};
		// Add default type mappings for built-in jni crate types
		let builtins = [
			("JByteBuffer", "java.nio.ByteBuffer", "objects::JByteBuffer", false),
			(
				"JCharSequence",
				"java.lang.CharSequence",
				"objects::JCharSequence",
				false,
			),
			("JClassLoader", "java.lang.ClassLoader", "objects::JClassLoader", false),
			("JClass", "java.lang.Class", "objects::JClass", true),
			("JCollection", "java.util.Collection", "objects::JCollection", false),
			("JIterator", "java.util.Iterator", "objects::JIterator", false),
			("JList", "java.util.List", "objects::JList", false),
			("JMap", "java.util.Map", "objects::JMap", false),
			("JMapEntry", "java.util.Map$Entry", "objects::JMapEntry", false),
			("JObject", "java.lang.Object", "objects::JObject", true),
			("JSet", "java.util.Set", "objects::JSet", false),
			(
				"JStackTraceElement",
				"java.lang.StackTraceElement",
				"objects::JStackTraceElement",
				false,
			),
			("JString", "java.lang.String", "objects::JString", true),
			("JThread", "java.lang.Thread", "objects::JThread", false),
			("JThrowable", "java.lang.Throwable", "objects::JThrowable", true),
		];

		for (simple_name, java_class_str, module_path, is_core) in builtins {
			let java_class = Rc::new(parse_java_class_dotted(java_class_str));

			let jni_full_path = format!("abos_jni::{module_path}");
			let real_full_path = format!("{jni_crate_str}::{module_path}");

			self.insert_ref_type(&jni_full_path, &real_full_path, (*java_class).clone(), true)
				.expect("Failed to insert built-in jni crate type mapping");
			self.insert_alias(simple_name, &jni_full_path)
				.expect("Failed to insert built-in jni crate type alias");
			if is_core {
				self.core_java.insert(java_class.clone());
			}
		}
	}

	// Lookup the canonical RustType for a Reference type path
	pub(super) fn rust_reference_type(&mut self, path: &str, is_builtin: bool) -> Rc<RustType> {
		let candidate = RustType {
			target: RustTypeTarget::Reference {
				path: path.to_string(),
				is_builtin,
			},
		};
		if let Some(existing) = self.rust_types.get(&candidate) {
			// It would be a bug if a builtin type resolved to a non-builtin existing type
			if is_builtin {
				assert!(
					existing.is_builtin(),
					"Inconsistent is_builtin flag for existing Rust reference type '{path}'"
				);
			}
			return existing.clone();
		}
		let rust_type = Rc::new(candidate);
		self.rust_types.insert(rust_type.clone());
		rust_type
	}

	// Lookup the canonical RustType for a Primitive type path
	pub(super) fn rust_prim_type(&mut self, primitive: PrimitiveType, path: String, is_builtin: bool) -> Rc<RustType> {
		let candidate = RustType {
			target: RustTypeTarget::Primitive {
				primitive,
				path,
				is_builtin,
			},
		};
		if let Some(existing) = self.rust_types.get(&candidate) {
			// It would be a bug if a builtin type resolved to a non-builtin existing type
			if is_builtin {
				assert!(
					existing.is_builtin(),
					"Inconsistent is_builtin flag for existing Rust primitive type '{}'",
					candidate.path()
				);
			}
			return existing.clone();
		}
		let rust_type = Rc::new(candidate);
		self.rust_types.insert(rust_type.clone());
		rust_type
	}
}
