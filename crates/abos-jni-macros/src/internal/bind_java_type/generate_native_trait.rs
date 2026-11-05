use super::*;

/// Generate all native method related code (trait, wrappers, registration)
/// Returns: (trait_and_impl_struct, registration_code)
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_native_methods_code(
	type_name: &Ident,
	api_name: &Ident,
	type_visibility: &VisibilitySpec,
	java_class_internal: &str,
	java_class_dotted: &str,
	native_methods: &[NativeMethod],
	native_trait_name: &Option<Ident>,
	type_mappings: &TypeMappings,
	native_methods_export: bool,
	native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	native_methods_error_policy: &Option<syn::Path>,
	jni: &syn::Path,
) -> Result<(TokenStream, TokenStream)> {
	// If there are no native methods, return empty
	if native_methods.is_empty() {
		return Ok((quote! {}, quote! {}));
	}

	// Generate default names if not provided
	let default_trait_name = format_ident!("{}NativeInterface", type_name);

	let trait_name = native_trait_name.as_ref().unwrap_or(&default_trait_name);

	// Generate the native trait
	let native_trait = generate_native_trait(
		trait_name,
		type_name,
		type_visibility,
		native_methods,
		type_mappings,
		jni,
	);

	// Generate wrapper methods (to be added to impl API block)
	let wrapper_methods = generate_native_wrappers(
		type_name,
		api_name,
		java_class_internal,
		native_methods,
		trait_name,
		type_mappings,
		native_methods_catch_unwind,
		default_abi_check,
		native_methods_error_policy,
		jni,
	);

	// Wrap the wrappers in an impl block for the API struct
	let wrapper_impl = quote! {
		impl #api_name {
			#wrapper_methods
		}
	};

	// Generate registration code (to be added to API::get())
	let registration_code = generate_native_registration_code(api_name, native_methods, type_mappings, jni)?;

	// Generate exported native method functions for methods marked with export = true
	let native_exports = generate_native_exports(
		type_name,
		api_name,
		java_class_dotted,
		native_methods,
		native_methods_export,
		type_mappings,
		jni,
	)?;

	// Combine trait, impl struct, wrapper impl, and exports for top-level output
	let trait_and_impl = quote! {
		#native_trait
		#wrapper_impl
		#native_exports
	};

	Ok((trait_and_impl, registration_code))
}

/// Generate the native methods trait
pub(super) fn generate_native_trait(
	trait_name: &Ident,
	type_name: &Ident,
	type_visibility: &VisibilitySpec,
	native_methods: &[NativeMethod],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let mut method_sigs = Vec::new();

	// Generate instance native method signatures
	for method in native_methods {
		// Skip methods with direct function bindings - they bypass the trait
		if method.native_fn.is_some() {
			continue;
		}

		let rust_name = &method.rust_name;
		let lifetime = quote! { 'local };

		let mut params = Vec::new();

		// Raw methods take EnvUnowned directly, regular methods take &mut Env
		if method.is_raw {
			params.push(quote! { unowned_env: #jni::EnvUnowned<'local> });
		} else {
			params.push(quote! { env: &mut #jni::Env<'local> });
		}

		if method.is_static {
			params.push(quote! { class: #jni::objects::JClass<'local> });
		} else {
			params.push(quote! { this: #type_name<'local> });
		}

		for param in &method.method_signature.parameters {
			let param_name = &param.name;
			// Use the native trait arg type (without AsRef)
			let rust_type = sig_type_to_rust_native_trait_arg_type(&param.ty, &lifetime, type_mappings, jni);
			params.push(quote! { #param_name: #rust_type });
		}

		let return_type =
			sig_type_to_rust_return_type(&method.method_signature.return_type, &lifetime, type_mappings, jni);

		// Only emit cfg attributes on trait methods (not doc or other attributes)
		let cfg_attrs = extract_cfg_attrs(&method.attrs);

		// Raw methods return the value directly, regular methods return Result
		if method.is_raw {
			method_sigs.push(quote! {
				#(#cfg_attrs)*
				fn #rust_name<'local>(#(#params),*) -> #return_type;
			});
		} else {
			method_sigs.push(quote! {
				#(#cfg_attrs)*
				fn #rust_name<'local>(#(#params),*) -> ::std::result::Result<#return_type, Self::Error>;
			});
		}
	}

	// If there are no methods that need trait implementations, don't generate the trait
	if method_sigs.is_empty() {
		return quote! {};
	}

	let trait_visibility = VisibilitySpec::PubCrate.clamp_to(type_visibility).to_tokens();
	quote! {
		/// Native methods trait for user implementation
		#trait_visibility trait #trait_name {
			type Error: From<#jni::errors::Error>;
			#(#method_sigs)*
		}
	}
}
