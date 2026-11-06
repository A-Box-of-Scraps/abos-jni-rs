use super::{
	ConcreteType, Constructor, IdLookup, Ident, Result, SigType, TokenStream, TypeMappings, extract_cfg_attrs,
	format_ident, generate_jni_call_args, lit_cstr_mutf8, quote, sig_type_to_rust_type_core,
};

/// Generate field and initialization code from `IdLookup` descriptors
pub(super) fn generate_id_fields_and_inits(
	lookups: Vec<IdLookup>,
	jni: &syn::Path,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
	let mut fields = Vec::new();
	let mut inits = Vec::new();

	for lookup in lookups {
		let field_name = lookup.field_name;
		let field_type = lookup.field_type;
		let java_name = &lookup.java_name;
		let signature = &lookup.signature;
		let lookup_fn = lookup.lookup_fn;
		let cfg_attrs = &lookup.cfg_attrs;

		// Create CStr literals for both the name and signature
		let name_cstr = lit_cstr_mutf8(java_name);

		// Add field to API struct with cfg guards
		fields.push(quote! {
			#(#cfg_attrs)*
			#field_name: #field_type,
		});

		// Add initialization with cfg guards
		inits.push(quote! {
            #(#cfg_attrs)*
            #field_name: env.#lookup_fn(class, #jni::strings::JNIStr::from_cstr_unchecked(#name_cstr), #jni::jni_sig!(jni=#jni, #signature))?,
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
	let mut lookups = Vec::new();

	for constructor in constructors {
		let name = &constructor.name;
		let method_id_field = format_ident!("{}_method_id", name);

		// Use the MethodSignature to generate JNI signature
		let jni_sig_str = constructor
			.method_signature
			.to_jni_signature(type_mappings)
			.map_err(|e| {
				syn::Error::new(
					name.span(),
					format!("Failed to generate JNI signature for constructor '{name}': {e}"),
				)
			})?;

		let cfg_attrs = extract_cfg_attrs(&constructor.attrs);

		lookups.push(IdLookup {
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

/// Convert a `JavaType` to a Rust native method trait argument type with lifetime
/// This variant does NOT use `AsRef` - it takes reference types by value
pub(super) fn sig_type_to_rust_native_trait_arg_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	// For native trait methods, use the concrete type directly without AsRef
	sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni)
}

/// Convert a `JavaType` to a Rust method argument type with lifetime
/// This variant uses impl `AsRef` for object and array types
pub(super) fn sig_type_to_rust_arg_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	// For primitives and void, return the type as-is (no AsRef needed)
	#[allow(clippy::collapsible_if)]
	if let SigType::Alias(alias_name) = sig_type {
		if let Some(ConcreteType::Primitive { .. }) = type_mappings.map_alias(alias_name) {
			return sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni);
		}
	}

	// For objects, arrays, and rust references, wrap in impl AsRef
	let concrete_type = sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni);
	quote! { impl AsRef<#concrete_type> }
}

/// Convert a `JavaType` to a Rust method return type with lifetime
pub(super) fn sig_type_to_rust_return_type(
	sig_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	// Return types are identical to the core type representation
	sig_type_to_rust_type_core(sig_type, lifetime, type_mappings, jni)
}

/// Generate constructor implementations
pub(super) fn generate_constructors(
	type_name: &Ident,
	api_name: &Ident,
	constructors: &[Constructor],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let mut constructor_impls = Vec::new();

	for constructor in constructors {
		let name = &constructor.name;
		let method_id_field = format_ident!("{}_method_id", name);

		// Generate JNI call arguments data
		let args = generate_jni_call_args(&constructor.method_signature.parameters, type_mappings, jni);

		let lifetimes = &args.lifetimes;
		let decls = &args.decls;
		let jvalue_conversions = &args.jvalue_conversions;

		// Add lifetime declarations for the function
		let lifetime_decls = if lifetimes.is_empty() {
			quote! { <'env_local> }
		} else {
			quote! { <'env_local, #(#lifetimes),*> }
		};

		// Generate explicit type annotation for jni_args array to handle empty case
		let jni_args_type = if jvalue_conversions.is_empty() {
			quote! { : [#jni::sys::jvalue; 0] }
		} else {
			quote! {}
		};

		// Apply attributes to the constructor
		let attrs = &constructor.attrs;

		constructor_impls.push(quote! {
			#(#attrs)*
			pub fn #name #lifetime_decls (
				env: &mut #jni::Env<'env_local>,
				#(#decls),*
			) -> #jni::errors::Result<#type_name<'env_local>> {
				let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
				let jni_args #jni_args_type = [#(#jvalue_conversions),*];

				unsafe {
					use #jni::refs::Reference as _;

					// NewObjectA is not exception safe, so we can't go ahead with the JNI call if
					// there's currently a pending exception.
					if env.exception_check() {
						return Err(#jni::errors::Error::JavaException);
					}

					// Since we're returning a new local reference we need to assert that the
					// lifetime we have is associated with the top local reference frame
					env.assert_top();

					let class: &#jni::objects::JClass = api.class.as_ref();

					// Call NewObjectA
					let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
					let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
					let ret_obj: #jni::sys::jobject = ((*interface).v1_1.NewObjectA)(
						env_ptr,
						class.as_raw(),
						api.#method_id_field.into_raw(),
						jni_args.as_ptr()
					);

					// Check for exception
					if env.exception_check() {
						return Err(#jni::errors::Error::JavaException);
					}

					Ok(#type_name::from_raw(env, ret_obj))
				}
			}
		});
	}

	if constructor_impls.is_empty() {
		Ok(quote! {})
	} else {
		Ok(quote! {
			impl #type_name<'static> {
				#(#constructor_impls)*
			}
		})
	}
}
