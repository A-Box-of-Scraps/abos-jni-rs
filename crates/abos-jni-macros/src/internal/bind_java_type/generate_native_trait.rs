use super::{
	AbiCheck, Ident, NativeMethod, Result, TokenStream, TypeMappings, VisibilitySpec, extract_cfg_attrs, format_ident,
	generate_native_exports, generate_native_registration_code, generate_native_wrappers, quote,
	sig_type_to_rust_native_trait_arg_type, sig_type_to_rust_return_type,
};

/// Generate all native method related code (trait, wrappers, registration)
/// Returns: (`trait_and_impl_struct`, `registration_code`)
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_native_methods_code(
	type_name: &Ident,
	api_name: &Ident,
	type_visibility: &VisibilitySpec,
	java_class_internal: &str,
	java_class_dotted: &str,
	native_methods: &[NativeMethod],
	native_trait_name: Option<&Ident>,
	type_mappings: &TypeMappings,
	native_methods_export: bool,
	native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	native_methods_error_policy: Option<&syn::Path>,
	jni: &syn::Path,
) -> Result<(TokenStream, TokenStream)> {
	if native_methods.is_empty() {
		return Ok((quote! {}, quote! {}));
	}

	let default_trait_name: Ident = format_ident!("{}NativeInterface", type_name);

	let trait_name: &Ident = native_trait_name.unwrap_or(&default_trait_name);

	let native_trait: TokenStream = generate_native_trait(
		trait_name,
		type_name,
		type_visibility,
		native_methods,
		type_mappings,
		jni,
	);

	let wrapper_methods: TokenStream = generate_native_wrappers(
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

	let wrapper_impl: TokenStream = quote! {
		impl #api_name {
			#wrapper_methods
		}
	};

	let registration_code: TokenStream =
		generate_native_registration_code(api_name, native_methods, type_mappings, jni)?;

	let native_exports: TokenStream = generate_native_exports(
		type_name,
		api_name,
		java_class_dotted,
		native_methods,
		native_methods_export,
		type_mappings,
		jni,
	)?;

	let trait_and_impl: TokenStream = quote! {
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
	let mut method_sigs: Vec<TokenStream> = Vec::new();

	for method in native_methods.iter().filter(|method| method.native_fn.is_none()) {
		method_sigs.push(generate_trait_signature(type_name, method, type_mappings, jni));
	}

	if method_sigs.is_empty() {
		return quote! {};
	}

	let trait_visibility: TokenStream = VisibilitySpec::PubCrate.clamp_to(type_visibility).to_tokens();
	quote! {
		/// Native methods trait for user implementation
		#trait_visibility trait #trait_name {
			type Error: From<#jni::errors::Error>;
			#(#method_sigs)*
		}
	}
}

fn generate_trait_signature(
	type_name: &Ident,
	method: &NativeMethod,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let rust_name: &Ident = &method.rust_name;
	let lifetime: TokenStream = quote! { 'local };

	let mut params: Vec<TokenStream> = Vec::new();

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

	for param in &method.signature.parameters {
		let param_name: &Ident = &param.name;
		let rust_type: TokenStream = sig_type_to_rust_native_trait_arg_type(&param.ty, &lifetime, type_mappings, jni);
		params.push(quote! { #param_name: #rust_type });
	}

	let return_type: TokenStream =
		sig_type_to_rust_return_type(&method.signature.return_type, &lifetime, type_mappings, jni);

	// Only emit cfg attributes on trait methods (not doc or other attributes)
	let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&method.attrs);

	if method.is_raw {
		quote! {
			#(#cfg_attrs)*
			fn #rust_name<'local>(#(#params),*) -> #return_type;
		}
	} else {
		quote! {
			#(#cfg_attrs)*
			fn #rust_name<'local>(#(#params),*) -> ::std::result::Result<#return_type, Self::Error>;
		}
	}
}
