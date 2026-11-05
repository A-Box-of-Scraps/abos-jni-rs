//! Generate Rust JNI bindings from Java sources, class files, JARs, and Android SDKs.
mod api;
mod internal;

pub use api::{
	AndroidSdk, ArgInfo, BindgenOptions, BindingInfo, Bindings, Builder, ClassInfo, Error, FieldInfo, HiddenApiFilter,
	InstanceOfInfo, MethodInfo, MethodSignature, Module, ModuleBinding, ModuleEvent, Result, TypeInfo, TypeMap,
};

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
