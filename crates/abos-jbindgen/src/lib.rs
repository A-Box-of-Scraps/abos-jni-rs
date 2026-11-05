//! Generate Rust JNI bindings from Java sources, class files, JARs, and Android SDKs.
//!
//! Generated identifiers use ASCII spelling: Rust keywords and the JNI macro's `non_null` qualifier gain a trailing underscore,
//! punctuation becomes underscores, and non-ASCII characters become `_u<hex>_`.
//! Original Java names are retained for JNI lookup. Parameter-name collisions gain
//! additional trailing underscores.
//!
//! The current `jni::bind_java_type!` macro cannot represent certain Java member
//! names, such as names containing `$`. Generation rejects these with an error;
//! use [`Builder::skip_signature`] to explicitly omit unsupported members.
//!
//! Bytecode JAR inputs use a base-only policy: module/package metadata and all
//! `META-INF/` entries (including multi-release variants) are excluded before
//! parsing or pattern matching. Remaining classes are processed in archive-name
//! order, independently of ZIP entry order and the host JVM. Version-only classes
//! and members are not generated; target-release selection is not currently supported.
//! The JVM still selects its runtime variant, which must retain the generated base members.
mod api;
mod internal;

pub use api::{
	AndroidSdk, ArgInfo, BindgenOptions, BindingInfo, Bindings, Builder, ClassInfo, Error, FieldInfo, HiddenApiFilter,
	InstanceOfInfo, MethodInfo, MethodSignature, Module, ModuleBinding, ModuleEvent, Result, TypeInfo, TypeMap,
};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
