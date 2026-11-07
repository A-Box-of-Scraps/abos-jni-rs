use super::{
	ConcreteType, Ident, JavaClassName, ParseStream, PrimitiveType, Rc, Result, RustType, RustTypeTarget, Span, Token,
	braced, path_to_string_no_spaces, typealias,
};

/// Type mappings from Rust Reference types to Java class names and aliases for
/// primitive types
///
/// Firstly, the type mappings start with a set of N:1 mapping from Rust type
/// aliases to `RustTypes` that either represent a `Primitive` type or a
/// `Reference` type.
///
/// E.g. `"jint"` => `{ Primitive(Int), abos_jni::sys::jint }`, `"i32"` => `{
///     Primitive(Int), i32 }`, `"JString"` =>
///     `Reference("abos_jni::objects::JString")`
///
/// ## Reference Types
///
///   A Reference type is represented by a full Rust type path (e.g.,
///   "`abos_jni::objects::JString`") (such as "`JString`" => "`abos_jni::objects::JString`")
///
///   Additionally, for the Reference types there are also:
///
/// - A N:1 mapping from canonical Rust type paths to Java class names (such as
///   "`abos_jni::objects::JString`" => "java.lang.String")
///
///   Note: that multiple Rust type paths can map to the same Java class name,
///   such as if a third-party crate provides it's own bindings for a java.lang
///   type that has a built-in binding in the jni crate. (like
///   JList/java.util.List)
///
///   Note: As a restriction, certain "core" java.lang classes cannot be
///   remapped, including java.lang.Object, java.lang.Class, java.lang.String,
///   java.lang.Throwable.
///
/// - A 1:1 mapping from Java class names to canonical Rust type paths (such as
///   "java.lang.String" => "`abos_jni::objects::JString`")
///
///   Note: When there are multiple Rust type paths that map to the same Java
///   class name, then the most-recently added mapping takes precedence in the
///   reverse lookup so that built-in (non-core) types can be overridden by
///   user-defined types.
///
/// ## Primitive Types
///
/// A primitive type is represented by the `PrimitiveType` enum, and a full path
/// to the `abos_jni::sys` primitive type (e.g., "`abos_jni::sys::jint`") or the name of an
/// equivalent Rust primitive type (e.g., "i32").
///
/// Each JNI primitive typically has two primitive `RustTypes`:
/// - The `abos_jni::sys` type (e.g., "`abos_jni::sys::jint`")
/// - The Rust primitive type (e.g., "i32")
///
/// and then one alias for the Java type name (e.g., "int") that points to the
/// `abos_jni::sys` type.
///
/// A `type_map` may add additional aliases for primitive types,
///
/// There are no other mappings for primitive types beyond the alias -> `RustType`
pub struct TypeMappings {
	/// Maps Rust type names to canonical Rust type paths
	///
	/// For example, built-in types have multiple aliases, so signatures can
	/// use types like "`JString`" or "`abos_jni::objects::JString`" for the same type.
	///
	/// Note: Even if the jni crate is renamed, the full-path alias for built-in
	/// types will still have a `abos_jni::` prefix, and it's only the
	/// `RustType::path` that will have the renamed crate path.
	alias_to_rust: std::collections::HashMap<String, Rc<RustType>>,
	/// Set of all known Rust types
	rust_types: std::collections::HashSet<Rc<RustType>>,
	/// Maps `RustType::path`s to Java class names
	///
	/// Only includes the canonical Rust type paths, so make sure to resolve
	/// aliases like "`JString`" before looking up in this map.
	rust_to_java: std::collections::HashMap<Rc<RustType>, Rc<JavaClassName>>,
	/// Reverse mapping from Java class names to the canonical `RustType` path
	java_to_rust: std::collections::HashMap<Rc<JavaClassName>, Rc<RustType>>,
	/// Set of core Java types that can not be mapped to non-jni-crate types
	///
	/// Although some types, like java.util.List, can be remapped to third-party
	/// bindings, we don't allow alternative mappings for core java.lang types
	/// like java.lang.Object, java.lang.Class, java.lang.String.
	///
	/// In addition to the types listed here, we also consider any array type
	/// (any descriptor that starts with '[') to be a core type that cannot be
	/// remapped.
	core_java: std::collections::HashSet<Rc<JavaClassName>>,
	/// The path to the jni crate (e.g., `jni` or `::jni` or `crate::jni`)
	jni_crate: syn::Path,
}

mod defaults;
mod lookup;
