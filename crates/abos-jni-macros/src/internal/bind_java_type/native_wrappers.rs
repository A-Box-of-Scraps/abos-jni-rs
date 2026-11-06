use super::{
	AbiCheck, Ident, NativeMethod, TokenStream, TypeMappings, extract_cfg_attrs, format_ident,
	generate_native_method_abi_check, quote, sig_type_to_rust_native_trait_arg_type, sig_type_to_rust_return_type,
};

/// Generate wrapper functions for native methods as API struct methods
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_native_wrappers(
	type_name: &Ident,
	api_name: &Ident,
	_java_class_internal: &str,
	native_methods: &[NativeMethod],
	trait_name: &Ident,
	type_mappings: &TypeMappings,
	default_native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	default_error_policy: Option<&syn::Path>,
	jni: &syn::Path,
) -> TokenStream {
	let mut wrappers = Vec::new();

	for method in native_methods {
		let wrapper = generate_single_native_wrapper(
			type_name,
			method,
			trait_name,
			api_name,
			type_mappings,
			default_native_methods_catch_unwind,
			default_abi_check,
			default_error_policy,
			jni,
		);
		wrappers.push(wrapper);
	}

	quote! {
		#(#wrappers)*
	}
}

/// Generate a single native method wrapper as an API struct method
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_single_native_wrapper(
	type_name: &Ident,
	method: &NativeMethod,
	trait_name: &Ident,
	api_name: &Ident,
	type_mappings: &TypeMappings,
	default_native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	default_error_policy: Option<&syn::Path>,
	jni: &syn::Path,
) -> TokenStream {
	let rust_name = &method.rust_name;
	let wrapper_name = format_ident!("{}_native_method", rust_name);

	let lifetime = quote! { 'local };

	// Build parameter list for extern "system" fn
	let mut params = Vec::new();
	params.push(quote! { mut unowned_env: #jni::EnvUnowned<#lifetime> });

	if method.is_static {
		params.push(quote! { class: #jni::objects::JClass<#lifetime> });
	} else {
		params.push(quote! { this: #type_name<#lifetime> });
	}

	// Add method parameters using parameter names from signature
	// Use native trait arg type (without AsRef) to match the trait signature
	for param in &method.signature.parameters {
		let param_name = &param.name;
		let rust_type = sig_type_to_rust_native_trait_arg_type(&param.ty, &lifetime, type_mappings, jni);
		params.push(quote! { #param_name: #rust_type });
	}

	// Get return type
	let return_type = sig_type_to_rust_return_type(&method.signature.return_type, &lifetime, type_mappings, jni);

	let need_abi_check = method.abi_check.unwrap_or(default_abi_check);
	let abi_check = generate_native_method_abi_check(
		jni,
		&method.java_name,
		need_abi_check,
		method.is_raw,
		method.is_static,
		None, // type mappings are checked in the TypeAPI::get() method
	);

	let with_env_api = if method.catch_unwind.unwrap_or(default_native_methods_catch_unwind) {
		quote! { with_env }
	} else {
		quote! { with_env_no_catch }
	};

	let wrapper_body = wrapper_body(
		method,
		api_name,
		trait_name,
		&abi_check,
		&with_env_api,
		default_error_policy,
		jni,
	);

	// Extract cfg attributes to apply to the wrapper function
	let cfg_attrs = extract_cfg_attrs(&method.attrs);

	quote! {
		#(#cfg_attrs)*
		#[allow(unused)]
		extern "system" fn #wrapper_name<#lifetime>(
			#(#params),*
		) -> #return_type {
			#wrapper_body
		}
	}
}

fn wrapper_body(
	method: &NativeMethod,
	api_name: &Ident,
	trait_name: &Ident,
	abi_check: &TokenStream,
	with_env_api: &TokenStream,
	default_error_policy: Option<&syn::Path>,
	jni: &syn::Path,
) -> TokenStream {
	let rust_name = &method.rust_name;
	let callable = method.native_fn.as_ref().map_or_else(
		|| quote! { <#api_name as #trait_name>::#rust_name },
		|path| quote! { #path },
	);
	let environment = if method.is_raw {
		quote! { unowned_env }
	} else {
		quote! { env }
	};
	let receiver = if method.is_static {
		quote! { class }
	} else {
		quote! { this }
	};
	let parameters = method.signature.parameters.iter().map(|param| &param.name);
	let call = quote! { #callable(#environment, #receiver, #(#parameters),*) };
	if method.is_raw {
		return quote! { #abi_check #call };
	}
	let error_policy = method.error_policy.as_ref().or(default_error_policy).map_or_else(
		|| quote! { #jni::errors::ThrowRuntimeExAndDefault },
		|policy| quote! { #policy },
	);
	quote! {
		unowned_env.#with_env_api(|env| { #abi_check #call }).resolve::<#error_policy>()
	}
}
