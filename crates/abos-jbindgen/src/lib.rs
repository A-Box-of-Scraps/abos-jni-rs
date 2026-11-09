#![doc = include_str!("../README.md")]

mod api;
mod internal;

pub use api::{
	AndroidSdk, ArgInfo, BindgenOptions, BindingInfo, Bindings, Builder, ClassInfo, Error, FieldInfo, HiddenApiFilter,
	InstanceOfInfo, MethodInfo, MethodSignature, Module, ModuleBinding, ModuleEvent, Result, TypeInfo, TypeMap,
};

/// Internal entry points for fuzz harnesses; not a supported public API.
#[cfg(feature = "_fuzzing")]
#[doc(hidden)]
pub mod fuzzing {
	pub use crate::internal::bytecode::{parse_class, parse_method_descriptor, parse_type_descriptor};
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
