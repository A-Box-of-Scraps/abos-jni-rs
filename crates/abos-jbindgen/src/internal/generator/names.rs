use super::identifiers::rust_identifier;
use crate::TypeMap;
use std::{borrow::Cow, collections::HashSet};
/// Use dot-separated binary names internally, without the macro's default-package marker.
pub fn canonical_java_name(name: &str) -> Cow<'_, str> {
	let name: &str = name.strip_prefix('.').unwrap_or(name);
	if name.contains('/') {
		Cow::Owned(name.replace('/', "."))
	} else {
		Cow::Borrowed(name)
	}
}

/// The binding macro requires a leading dot for a class in the default package.
pub fn macro_java_name(name: &str) -> String {
	let name: Cow<'_, str> = canonical_java_name(name);
	if name.contains('.') {
		name.into_owned()
	} else {
		format!(".{name}")
	}
}

pub struct BuiltinType {
	/// Simple Rust type name (e.g., "`JString`")
	pub(crate) rust_name: &'static str,
	/// Java fully-qualified type name (e.g., "java.lang.String")
	pub(crate) java_name: &'static str,
	/// Whether this is a core type that cannot be remapped
	pub(crate) is_core: bool,
}

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

/// Core wrappers are supplied by abos-jni and cannot be declared or remapped.
pub fn core_jni_type(java_name: &str) -> Option<&'static BuiltinType> {
	let java_name: Cow<'_, str> = canonical_java_name(java_name);
	builtin_jni_types()
		.iter()
		.find(|builtin| builtin.is_core && builtin.java_name == java_name)
}

/// Remove inner-class separators, apply the optional prefix, and normalize as a Rust identifier.
pub fn derive_rust_type_name(class_name: &str, rust_type_override: Option<&str>, name_prefix: Option<&str>) -> String {
	let class_part: &str = class_name.split('/').next_back().unwrap_or(class_name);

	let base_name: &str = rust_type_override.unwrap_or(class_part);

	let base_name_cleaned: String = base_name.replace('$', "");

	rust_identifier(&format!("{}{base_name_cleaned}", name_prefix.unwrap_or_default()))
}

// Match abos-jni-macros naming rules so shorthand bindings resolve the original Java name.
pub fn jni_macros_snake_case_to_lower_camel_case(s: &str) -> String {
	// Preserve intentional mixed or uppercase naming.
	if s.chars().any(char::is_uppercase) {
		return s.to_string();
	}

	let leading_underscores: usize = s.chars().take_while(|&c| c == '_').count();

	let trailing_underscores: usize = s.chars().rev().take_while(|&c| c == '_').count();

	if leading_underscores + trailing_underscores >= s.len() {
		return s.to_string();
	}

	let middle: &str = &s[leading_underscores..s.len() - trailing_underscores];

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
	let has_letter: bool = java_name.chars().any(char::is_alphabetic);
	let is_upper_snake_case: bool = has_letter
		&& java_name
			.chars()
			.all(|c| c.is_uppercase() || c == '_' || c.is_numeric());

	if is_upper_snake_case {
		// The macro preserves uppercase names, so unchanged identifiers are reversible.
		let name: String = rust_identifier(java_name);
		let reversible: bool = name == java_name;
		return (name, reversible);
	}

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

	let result: String = rust_identifier(&result);
	// Check reversibility after normalization so renamed members use explicit Java names.
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
	let java_name: Cow<'_, str> = canonical_java_name(&type_info.name);
	let base_type: String = type_map.get_rust_type(&java_name).map_or_else(
		|| qualified_name_to_jni_type(&java_name).unwrap_or_else(|| format!("\"{}\"", macro_java_name(&java_name))),
		|rust| {
			used_types.insert(java_name.to_string());
			rust.to_owned()
		},
	);

	if type_info.array_dimensions > 0 {
		format!("{}{}", base_type, "[]".repeat(type_info.array_dimensions))
	} else {
		base_type
	}
}

/// Convert a Java qualified name to a JNI Rust type if it's a known built-in
pub fn qualified_name_to_jni_type(qualified_name: &str) -> Option<String> {
	for builtin in builtin_jni_types() {
		if builtin.java_name == qualified_name {
			return Some(builtin.rust_name.to_string());
		}
	}

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
