use crate::TypeMap;
use std::collections::HashSet;
/// Builtin JNI type information
pub struct BuiltinType {
	/// Simple Rust type name (e.g., "`JString`")
	pub(crate) rust_name: &'static str,
	/// Java fully-qualified type name (e.g., "java.lang.String")
	pub(crate) java_name: &'static str,
	/// Whether this is a core type that cannot be remapped
	pub(crate) is_core: bool,
}

/// Get the list of builtin JNI types from the jni crate
pub const fn builtin_jni_types() -> &'static [BuiltinType] {
	&[
		BuiltinType {
			rust_name: "JByteBuffer",
			java_name: "java.nio.ByteBuffer",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JClassLoader",
			java_name: "java.lang.ClassLoader",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JClass",
			java_name: "java.lang.Class",
			is_core: true,
		},
		BuiltinType {
			rust_name: "JCollection",
			java_name: "java.util.Collection",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JIterator",
			java_name: "java.util.Iterator",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JList",
			java_name: "java.util.List",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JMap",
			java_name: "java.util.Map",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JMapEntry",
			java_name: "java.util.Map$Entry",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JObject",
			java_name: "java.lang.Object",
			is_core: true,
		},
		BuiltinType {
			rust_name: "JSet",
			java_name: "java.util.Set",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JStackTraceElement",
			java_name: "java.lang.StackTraceElement",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JString",
			java_name: "java.lang.String",
			is_core: true,
		},
		BuiltinType {
			rust_name: "JThread",
			java_name: "java.lang.Thread",
			is_core: false,
		},
		BuiltinType {
			rust_name: "JThrowable",
			java_name: "java.lang.Throwable",
			is_core: true,
		},
	]
}

/// Derive a Rust type name from a Java class simple name, applying transformations
/// to handle inner classes and applying optional prefix.
///
/// - Strips '$' characters used for inner classes (e.g., "ColorSpace$Rgb" -> "`ColorSpaceRgb`")
/// - Applies optional name prefix
pub fn derive_rust_type_name(class_name: &str, rust_type_override: Option<&str>, name_prefix: Option<&str>) -> String {
	// Extract the class portion from class_name (strip package)
	// For "android/os/Build$Partition" -> "Build$Partition"
	let class_part: &str = class_name.split('/').next_back().unwrap_or(class_name);

	let base_name: &str = rust_type_override.unwrap_or(class_part);

	// Strip '$' characters used for inner classes (e.g., "Build$Partition" -> "BuildPartition")
	let base_name_cleaned: String = base_name.replace('$', "");

	// Apply name prefix if specified
	if let Some(prefix) = name_prefix {
		format!("{prefix}{base_name_cleaned}")
	} else {
		base_name_cleaned
	}
}

// Match jni-macros naming rules so shorthand bindings resolve the original Java name.
pub fn jni_macros_snake_case_to_lower_camel_case(s: &str) -> String {
	// If the string contains any uppercase letters, assume it's intentionally cased
	// and return it unchanged
	if s.chars().any(char::is_uppercase) {
		return s.to_string();
	}

	// Find leading underscores
	let leading_underscores: usize = s.chars().take_while(|&c| c == '_').count();

	// Find trailing underscores
	let trailing_underscores: usize = s.chars().rev().take_while(|&c| c == '_').count();

	// If the entire string is underscores, return as-is
	if leading_underscores + trailing_underscores >= s.len() {
		return s.to_string();
	}

	// Extract the middle part (without leading/trailing underscores)
	let middle: &str = &s[leading_underscores..s.len() - trailing_underscores];

	// Convert the middle part
	let mut result = String::new();
	let mut capitalize_next = false;

	for c in middle.chars() {
		if c == '_' {
			capitalize_next = true;
		} else if capitalize_next && !c.is_ascii_digit() {
			result.extend(c.to_uppercase());
			capitalize_next = false;
		} else {
			result.push(c);
		}
	}

	// Reconstruct with leading and trailing underscores
	// Remove one leading underscore (if any exist)
	let adjusted_leading_underscores: usize = if leading_underscores > 0 {
		leading_underscores - 1
	} else {
		0
	};

	let mut final_result: String = String::with_capacity(s.len());
	for _ in 0..adjusted_leading_underscores {
		final_result.push('_');
	}
	final_result.push_str(&result);
	for _ in 0..trailing_underscores {
		final_result.push('_');
	}

	final_result
}

/// Convert a Java method name to Rust `snake_case`
///
/// Returns (`rust_name`, `is_reversible`) where `is_reversible` indicates whether
/// `jni_macros` can automatically derive the Java name from the Rust name
pub fn java_name_to_rust(java_name: &str) -> (String, bool) {
	// Check if the name is already in UPPER_SNAKE_CASE (constant style)
	// UPPER_SNAKE_CASE is detected when:
	// - All letters are uppercase
	// - May contain underscores and digits
	// - Has at least one letter
	let has_letter: bool = java_name.chars().any(char::is_alphabetic);
	let is_upper_snake_case: bool = has_letter
		&& java_name
			.chars()
			.all(|c| c.is_uppercase() || c == '_' || c.is_numeric());

	if is_upper_snake_case {
		// Keep UPPER_SNAKE_CASE names as-is (e.g., DEFAULT_KEYS_DIALER, RESULT_OK)
		// These are reversible since jni_macros leaves them unchanged
		return (java_name.to_string(), true);
	}

	// Simple conversion: insert underscores before uppercase letters
	// and convert to lowercase
	let mut result = String::new();
	let mut prev_was_lower = false;

	for (i, ch) in java_name.chars().enumerate() {
		if ch.is_uppercase() {
			if i > 0 && prev_was_lower {
				result.push('_');
			}
			result.push(ch.to_lowercase().next().unwrap_or(ch));
			prev_was_lower = false;
		} else {
			result.push(ch);
			prev_was_lower = ch.is_lowercase();
		}
	}

	// Check if the transformation is reversible by applying jni_macros algorithm
	let back_converted: String = jni_macros_snake_case_to_lower_camel_case(&result);
	let is_reversible: bool = back_converted == java_name;

	(result, is_reversible)
}

/// Resolve a type using `TypeInfo`, preferring `type_map` lookups, and track dependencies
pub fn resolve_type_with_deps(
	type_info: &crate::api::types::TypeInfo,
	type_map: &TypeMap,
	used_types: &mut HashSet<String>,
) -> String {
	// Get the base/element type first
	let base_type: String = type_map.get_rust_type(&type_info.name).map_or_else(
		|| qualified_name_to_jni_type(&type_info.name).unwrap_or_else(|| format!("\"{}\"", type_info.name)),
		|rust| {
			used_types.insert(type_info.name.clone());
			rust.to_owned()
		},
	);

	// Add array brackets if needed
	if type_info.array_dimensions > 0 {
		// For builtin types like JString, format as JString[]
		// For quoted types like "android.os.Build", format as "android.os.Build"[]
		format!("{}{}", base_type, "[]".repeat(type_info.array_dimensions))
	} else {
		base_type
	}
}

/// Convert a Java qualified name to a JNI Rust type if it's a known built-in
pub fn qualified_name_to_jni_type(qualified_name: &str) -> Option<String> {
	// Check builtin JNI types
	for builtin in builtin_jni_types() {
		if builtin.java_name == qualified_name {
			return Some(builtin.rust_name.to_string());
		}
	}

	// Java primitive types (for qualified names from source)
	match qualified_name {
		"boolean" => Some("jboolean".to_string()),
		"byte" => Some("jbyte".to_string()),
		"char" => Some("jchar".to_string()),
		"short" => Some("jshort".to_string()),
		"int" => Some("jint".to_string()),
		"long" => Some("jlong".to_string()),
		"float" => Some("jfloat".to_string()),
		"double" => Some("jdouble".to_string()),
		"void" => Some("void".to_string()),
		_ => None,
	}
}
