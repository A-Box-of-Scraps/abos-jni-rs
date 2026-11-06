//! Procedural macro for binding Java classes to Rust types
//!
//! This module provides a `bind_java_type!` macro that generates Reference type wrappers
//! and API bindings for Java classes, including constructors, methods, fields, and
//! native method registration.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
	Ident, LitBool, LitStr, Result, Token, braced, custom_keyword,
	ext::IdentExt,
	parenthesized,
	parse::{Parse, ParseStream},
};

use crate::internal::{
	mangle::{create_jni_fn_name, snake_case_to_lower_camel_case},
	native_method::{NativeMethodExport, generate_native_method_abi_check},
	signature::{parse_field_sig, sig},
	types::{AbiCheck, generate_type_mapping_checks, sig_type_to_rust_type_core},
};
use crate::internal::{
	signature::parse_method_sig,
	types::{JavaClassName, PrimitiveType, SigType, TypeMappings, parse_type},
};
use crate::internal::{
	signature::{FieldSignature, MethodSignature, parse_parameter_with_index},
	types::path_to_string_no_spaces,
};
use crate::internal::{str::lit_cstr_mutf8, types::ConcreteType};

// Custom keywords
custom_keyword!(jni);
custom_keyword!(rust_type);
custom_keyword!(rust_type_vis);
custom_keyword!(java_type);
custom_keyword!(type_map);
custom_keyword!(api);
custom_keyword!(native_trait);
custom_keyword!(constructors);
custom_keyword!(methods);
custom_keyword!(native_methods);
custom_keyword!(fields);
custom_keyword!(hooks);
custom_keyword!(name);
custom_keyword!(get);
custom_keyword!(set);
custom_keyword!(error);
custom_keyword!(native_methods_error_policy);
custom_keyword!(error_policy);
custom_keyword!(abi_check);
custom_keyword!(native_methods_export);
custom_keyword!(export);
custom_keyword!(native_methods_catch_unwind);
custom_keyword!(catch_unwind);
custom_keyword!(priv_type);
custom_keyword!(is_instance_of);
custom_keyword!(__jni_core);
custom_keyword!(raw);
custom_keyword!(non_null);

/// Represents a visibility modifier
#[derive(Clone)]
enum VisibilitySpec {
	Public,
	PubSelf,
	PubCrate,
	PubSuper,
	PubInPath(syn::Path),
}

impl std::fmt::Debug for VisibilitySpec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Public => write!(f, "Public"),
			Self::PubSelf => write!(f, "PubSelf"),
			Self::PubCrate => write!(f, "PubCrate"),
			Self::PubSuper => write!(f, "PubSuper"),
			Self::PubInPath(_) => write!(f, "PubInPath(..)"),
		}
	}
}

impl VisibilitySpec {
	fn to_tokens(&self) -> TokenStream {
		match self {
			Self::Public => quote! { pub },
			Self::PubSelf => quote! { pub(self) },
			Self::PubCrate => quote! { pub(crate) },
			Self::PubSuper => quote! { pub(super) },
			Self::PubInPath(path) => quote! { pub(in #path) },
		}
	}
	// Get back a visibility that is at most as permissive as `other`
	//
	// This is used to clamp the visibility of the `*API` struct to the visibility of the binding
	// type itself.
	fn clamp_to(&self, other: &Self) -> Self {
		match (self, other) {
			(Self::Public, _) => other.clone(),
			(Self::PubCrate, Self::Public) => Self::PubCrate,
			(Self::PubCrate, _) => other.clone(),
			(Self::PubSuper, Self::Public | Self::PubCrate) => Self::PubSuper,
			(Self::PubSuper, _) => other.clone(),
			(Self::PubSelf, Self::Public | Self::PubCrate | Self::PubSuper) => Self::PubSelf,
			(Self::PubSelf, _) => other.clone(),
			// XXX: it's a bit hand-wavey deciding how we should clamp `pub(in
			// path)`, but essentially we'll assume that if someone want's their
			// binding to only be visible for a specific module path then their
			// API type should also only be visible for the same path, unless
			// `other` is restricted to `pub(self)`
			(Self::PubInPath(_), Self::PubSelf) => Self::PubSelf,
			(Self::PubInPath(_), _) => self.clone(),
		}
	}
}

/// Represents a constructor definition
#[derive(Clone)]
struct Constructor {
	name: Ident,
	method_signature: MethodSignature,
	attrs: Vec<syn::Attribute>,
}

/// Represents a method definition
#[derive(Clone)]
struct Method {
	visibility: VisibilitySpec,
	java_name: String,
	rust_name: Ident,
	method_signature: MethodSignature,
	attrs: Vec<syn::Attribute>,
	is_static: bool,
	non_null: bool,
}

/// Represents a field definition
#[derive(Clone)]
struct Field {
	java_name: String,
	#[allow(dead_code)]
	rust_name: Ident,
	getter_name: Option<Ident>,
	setter_name: Option<Ident>,
	getter_visibility: VisibilitySpec,
	setter_visibility: VisibilitySpec,
	field_signature: FieldSignature,
	attrs: Vec<syn::Attribute>,
	getter_attrs: Vec<syn::Attribute>,
	setter_attrs: Vec<syn::Attribute>,
	is_static: bool,
	non_null: bool,
}

/// Represents an entry in the `is_instance_of` list
#[derive(Clone)]
struct IsInstanceOfEntry {
	/// The Rust type alias (e.g., "`JObject`" or "`abos_jni::objects::JObject`")
	type_alias: String,
	/// Optional stem name for the generated as_<stem>() method
	stem: Option<String>,
}

/// Represents a native method definition
#[derive(Clone)]
struct NativeMethod {
	java_name: String,
	rust_name: Ident,
	method_signature: MethodSignature,
	error_policy: Option<syn::Path>,
	export: NativeMethodExport,
	attrs: Vec<syn::Attribute>,
	/// Optional direct function binding (bypasses trait implementation)
	native_fn: Option<syn::Path>,
	/// Whether this is a raw JNI function (takes `EnvUnowned` directly, no `with_env` wrapper)
	is_raw: bool,
	/// Whether this is a static method
	is_static: bool,
	abi_check: Option<AbiCheck>,
	catch_unwind: Option<bool>,
}

/// The kind of method being parsed (determines validation rules)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodKind {
	Constructor,
	Method,
	NativeMethod,
}

/// Intermediate representation for a parsed method (after validation)
struct ParsedMethod {
	rust_name: Ident,
	java_name: String,                 // Always set (either explicit or derived from rust_name)
	method_signature: MethodSignature, // Always present after parsing
	visibility: Option<VisibilitySpec>,
	error_policy: Option<syn::Path>,    // Only set for NativeMethod
	export: Option<NativeMethodExport>, // Only set for NativeMethod
	attrs: Vec<syn::Attribute>,
	native_fn: Option<syn::Path>, // Only set for NativeMethod
	is_raw: bool,                 // Only set for NativeMethod
	is_static: bool,              // Set for both Method and NativeMethod
	abi_check: Option<AbiCheck>,  // Only set for NativeMethod
	catch_unwind: Option<bool>,   // Only set for NativeMethod
	non_null: bool,               // Set for Method and NativeMethod (not Constructor)
}

/// The main input structure for `bind_java_type`!
struct BindClassInput {
	type_name: Ident,
	type_visibility: Option<VisibilitySpec>,
	type_attrs: Vec<syn::Attribute>,
	java_class: JavaClassName,
	api_name: Option<Ident>,
	priv_type: Option<Ident>,
	native_trait_name: Option<Ident>,
	is_instance_of: Vec<IsInstanceOfEntry>,
	type_mappings: TypeMappings,
	constructors: Vec<Constructor>,
	methods: Vec<Method>,
	fields: Vec<Field>,
	native_methods: Vec<NativeMethod>,
	load_class_closure: Option<TokenStream>,
	init_priv_closure: Option<TokenStream>,
	native_methods_export: bool,
	native_methods_catch_unwind: bool,
	abi_check: AbiCheck,
	native_methods_error_policy: Option<syn::Path>,
	jni_core: bool,
	jni_crate: syn::Path,
	sys_type: Option<Ident>,
}

/// Intermediate representation for ID lookups (method IDs or field IDs)
struct IdLookup {
	/// The field name in the API struct (e.g., "`new_method_id`", "`get_value_field_id`")
	field_name: Ident,
	/// The field type (`JMethodID` or `JFieldID`)
	field_type: TokenStream,
	/// The Java name to look up (e.g., "<init>", "getValue", "myField")
	java_name: String,
	/// The JNI signature string
	signature: String,
	/// The function to call for lookup (e.g., "`get_method_id`", "`get_static_field_id`")
	lookup_fn: Ident,
	/// The cfg attributes to apply to this ID (e.g., #[cfg(feature = "foo")])
	cfg_attrs: Vec<syn::Attribute>,
}

/// Helper struct to hold processed JNI call arguments data
struct JniCallArgs {
	lifetimes: Vec<TokenStream>,
	jvalue_conversions: Vec<TokenStream>,
	decls: Vec<TokenStream>,
}

mod parse_method;
use parse_method::{parse_constructors, parse_methods, parse_name_spec, parse_native_methods, parse_visibility};

mod parse_field;
use parse_field::parse_fields;

mod parse_class;

mod expand;
pub use expand::*;

mod generate_api;
use generate_api::{generate_api_get_method, generate_api_struct, generate_type_struct};

mod references;
use references::{
	extract_cfg_attrs, generate_base_methods, generate_is_instance_of_assignable_checks, generate_is_instance_of_code,
	generate_reference_impl,
};

mod generate_constructors;
use generate_constructors::{
	generate_constructor_method_ids, generate_constructors, generate_id_fields_and_inits, sig_type_to_rust_arg_type,
	sig_type_to_rust_native_trait_arg_type, sig_type_to_rust_return_type,
};

mod ids;
use ids::{generate_field_ids, generate_method_ids};

mod calls;
use calls::{generate_jni_call_args, generate_methods};

mod field_access;
use field_access::{generate_jni_get_field_call, generate_jni_set_field_call};

mod generate_fields;
use generate_fields::generate_fields;

mod generate_native_trait;
use generate_native_trait::generate_native_methods_code;

mod native_wrappers;
use native_wrappers::generate_native_wrappers;

mod native_exports;
use native_exports::{generate_native_exports, generate_native_registration_code};
