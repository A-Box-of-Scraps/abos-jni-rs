//! Generate Rust JNI bindings for `abos-jni` from Java sources, class files, JARs, and Android SDKs.
//!
//! Generated identifiers use ASCII spelling: Rust keywords and the JNI macro's `non_null` qualifier gain a trailing underscore,
//! punctuation becomes underscores, and non-ASCII characters become `_u<hex>_`.
//! Original Java names are retained for JNI lookup. Parameter-name collisions gain
//! additional trailing underscores.
//!
//! Java member names containing `$` are preserved for JNI lookup while their
//! Rust bindings use normalized identifiers.
//!
//! Bytecode JAR inputs use a base-only policy: module/package metadata and all
//! `META-INF/` entries (including multi-release variants) are excluded before
//! parsing or pattern matching. Remaining classes are processed in archive-name
//! order, independently of ZIP entry order and the host JVM. Version-only classes
//! and members are not generated; target-release selection is not currently supported.
//! The JVM still selects its runtime variant, which must retain the generated base members.
//!
//! Android generation fails on selected-class parsing or generation errors instead
//! of silently returning incomplete bindings. Class failures retain their context
//! and underlying cause. There is no partial-output mode; class patterns and member
//! skips remain explicit ways to select the API surface.
mod api;
mod internal;

pub use api::{
	AndroidSdk, ArgInfo, BindgenOptions, BindingInfo, Bindings, Builder, ClassInfo, Error, FieldInfo, HiddenApiFilter,
	InstanceOfInfo, MethodInfo, MethodSignature, Module, ModuleBinding, ModuleEvent, Result, TypeInfo, TypeMap,
};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
