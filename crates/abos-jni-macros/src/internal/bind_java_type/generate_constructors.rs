use super::{
	ConcreteType, Constructor, IdLookup, Ident, Result, SigType, TokenStream, TypeMappings, extract_cfg_attrs,
	format_ident, generate_jni_call_args, lit_cstr_mutf8, quote, sig_type_to_rust_type_core,
};

/// Generate field and initialization code from `IdLookup` descriptors
pub(super) fn generate_id_fields_and_inits(
	lookups: Vec<IdLookup>,
	jni: &syn::Path,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
	let mut fields: Vec<TokenStream> = Vec::new();
	let mut inits: Vec<TokenStream> = Vec::new();

	for lookup in lookups {
		let field_name: Ident = lookup.field_name;
		let field_type: TokenStream = lookup.requirement.id_type(&lookup.field_type);
		let java_name: &String = &lookup.java_name;
		let signature: &String = &lookup.signature;
		let lookup_fn: Ident = lookup.lookup_fn;
		let cfg_attrs: &Vec<syn::Attribute> = &lookup.cfg_attrs;

		let name_cstr: syn::LitCStr = lit_cstr_mutf8(java_name);

		fields.push(quote! {
			#(#cfg_attrs)*
			#field_name: #field_type,
		});

		let lookup_call: TokenStream = quote! {
			env.#lookup_fn(class, #jni::strings::JNIStr::from_cstr_unchecked(#name_cstr), #jni::jni_sig!(jni=#jni, #signature))
		};
		let initializer: TokenStream = lookup.requirement.initialize_id(&lookup_call, jni);
		inits.push(quote! {
			#(#cfg_attrs)*
			#field_name: #initializer,
		});
	}

	(fields, inits)
}

/// Generate constructor method ID fields and initialization code
pub(super) fn generate_constructor_method_ids(
	constructors: &[Constructor],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<(Vec<TokenStream>, Vec<TokenStream>)> {
	let mut lookups: Vec<IdLookup> = Vec::new();

	for constructor in constructors {
		let name: &Ident = &constructor.name;
		let method_id_field: Ident = format_ident!("{}_method_id", name);

		let jni_sig_str: String = constructor.signature.to_jni_signature(type_mappings).map_err(|e| {
			syn::Error::new(
				name.span(),
				format!("Failed to generate JNI signature for constructor '{name}': {e}"),
			)
		})?;

		let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&constructor.attrs);

		lookups.push(IdLookup {
			requirement: constructor.requirement.clone(),
			field_name: method_id_field,
			field_type: quote! { #jni::ids::JMethodID },
			java_name: "<init>".to_string(),
			signature: jni_sig_str,
			lookup_fn: format_ident!("get_method_id"),
			cfg_attrs,
		});
	}

	Ok(generate_id_fields_and_inits(lookups, jni))
}

/// Convert a `SigType` to a native trait argument, taking reference types by value.
pub(super) fn sig_type_to_rust_native_trait_arg_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni)
}

/// Convert a `SigType` to a Rust argument, using `impl AsRef` for objects and arrays.
pub(super) fn sig_type_to_rust_arg_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	if let SigType::Alias(alias_name) = sig_type
		&& let Some(ConcreteType::Primitive { .. }) = type_mappings.map_alias(alias_name)
	{
		return sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni);
	}

	let concrete_type: TokenStream = sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni);
	quote! { impl AsRef<#concrete_type> }
}

/// Convert a `SigType` to a Rust return type with a lifetime.
pub(super) fn sig_type_to_rust_return_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni)
}

/// Generate constructor implementations.
pub(super) fn generate_constructors(
	type_name: &Ident,
	api_name: &Ident,
	constructors: &[Constructor],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	if constructors.is_empty() {
		return quote! {};
	}
	let implementations: TokenStream = constructors
		.iter()
		.map(|constructor| generate_constructor(type_name, api_name, constructor, type_mappings, jni))
		.collect();
	quote! { impl #type_name<'static> { #implementations } }
}

fn generate_constructor(
	type_name: &Ident,
	api_name: &Ident,
	constructor: &Constructor,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let name: &Ident = &constructor.name;
	let method_id_field: Ident = format_ident!("{}_method_id", name);
	let local_id: Ident = super::Requirement::local_id(&method_id_field);

	let args: crate::internal::bind_java_type::JniCallArgs =
		generate_jni_call_args(&constructor.signature.parameters, type_mappings, jni);

	let id_access: TokenStream = constructor.requirement.access_id(&method_id_field, "<init>", jni);
	let validation: TokenStream = args.reference_validation(&local_id, false, jni);
	let lifetimes: &Vec<TokenStream> = &args.lifetimes;
	let decls: &Vec<TokenStream> = &args.decls;
	let jvalue_conversions: &Vec<TokenStream> = &args.jvalue_conversions;

	let lifetime_decls: TokenStream = if lifetimes.is_empty() {
		quote! { <'env_local> }
	} else {
		quote! { <'env_local, #(#lifetimes),*> }
	};

	// Empty argument arrays need an explicit element type.
	let jni_args_type: TokenStream = if jvalue_conversions.is_empty() {
		quote! { : [#jni::sys::jvalue; 0] }
	} else {
		quote! {}
	};

	let attrs: &Vec<syn::Attribute> = &constructor.attrs;

	quote! {
		#(#attrs)*
		pub fn #name #lifetime_decls (
			env: &mut #jni::Env<'env_local>,
			#(#decls),*
		) -> #jni::errors::Result<#type_name<'env_local>> {
			let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
			#id_access
			#validation
			let jni_args #jni_args_type = [#(#jvalue_conversions),*];

			unsafe {
				use #jni::refs::Reference as _;

				// NewObjectA must not run with a pending exception.
				if env.exception_check() {
					return Err(#jni::errors::Error::JavaException);
				}

				// The new local reference must belong to the top frame.
				env.assert_top();

				let class: &#jni::objects::JClass = api.class.as_ref();

				let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
				let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
				let ret_obj: #jni::sys::jobject = ((*interface).v1_1.NewObjectA)(
					env_ptr,
					class.as_raw(),
					#local_id.into_raw(),
					jni_args.as_ptr()
				);

				if env.exception_check() {
					return Err(#jni::errors::Error::JavaException);
				}

				Ok(#type_name::from_raw(env, ret_obj))
			}
		}
	}
}
