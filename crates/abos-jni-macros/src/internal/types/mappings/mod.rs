use super::{
	ConcreteType, Ident, JavaClassName, ParseStream, PrimitiveType, Rc, Result, RustType, RustTypeTarget, Span, Token,
	braced, path_to_string_no_spaces, typealias,
};

/// Rust type aliases, Java class mappings, and primitive type mappings.
///
/// Aliases resolve to a canonical `RustType`. Multiple reference types may map to the
/// same Java class; reverse lookups select the most recently added mapping. Core Java
/// classes cannot be remapped.
///
/// Primitive aliases resolve directly to a primitive kind and Rust path. For example,
/// `i32` retains its Rust spelling, while `int` resolves to `abos_jni::sys::jint`.
pub struct TypeMappings {
	/// Aliases such as `JString` and `abos_jni::objects::JString` share a canonical type.
	/// Full-path built-in aliases retain the `abos_jni` prefix after a crate rename;
	/// only the resolved Rust path uses the override.
	alias_to_rust: std::collections::HashMap<String, Rc<RustType>>,
	rust_types: std::collections::HashSet<Rc<RustType>>,
	/// Canonical reference types to Java classes; aliases must be resolved first.
	rust_to_java: std::collections::HashMap<Rc<RustType>, Rc<JavaClassName>>,
	java_to_rust: std::collections::HashMap<Rc<JavaClassName>, Rc<RustType>>,
	/// Core Java classes that cannot be remapped to third-party types.
	core_java: std::collections::HashSet<Rc<JavaClassName>>,
	jni_crate: syn::Path,
}

mod defaults;
mod lookup;
