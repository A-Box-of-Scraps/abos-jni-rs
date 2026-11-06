use super::{
	AbiCheck, Ident, NativeMethod, TokenStream, TypeMappings, extract_cfg_attrs, format_ident,
	generate_native_method_abi_check, quote, sig_type_to_rust_native_trait_arg_type, sig_type_to_rust_return_type,
};

/// Generate wrapper functions for native methods as API struct methods
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_native_wrappers(
	type_name: &Ident,
	api_name: &Ident,
	_java_class_internal: &str,
	native_methods: &[NativeMethod],
	trait_name: &Ident,
	type_mappings: &TypeMappings,
	default_native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	default_error_policy: &Option<syn::Path>,
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
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_single_native_wrapper(
	type_name: &Ident,
	method: &NativeMethod,
	trait_name: &Ident,
	api_name: &Ident,
	type_mappings: &TypeMappings,
	default_native_methods_catch_unwind: bool,
	default_abi_check: AbiCheck,
	default_error_policy: &Option<syn::Path>,
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
	for param in &method.method_signature.parameters {
		let param_name = &param.name;
		let rust_type = sig_type_to_rust_native_trait_arg_type(&param.ty, &lifetime, type_mappings, jni);
		params.push(quote! { #param_name: #rust_type });
	}

	// Get return type
	let return_type = sig_type_to_rust_return_type(&method.method_signature.return_type, &lifetime, type_mappings, jni);

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

	// Generate the wrapper body based on native_fn and is_raw combination
	let wrapper_body = match (&method.native_fn, method.is_raw) {
		// Direct raw function (fn = raw path)
		(Some(raw_fn), true) => {
			// Call the function directly with unowned env - no with_env wrapper
			let mut call_args = vec![quote! { unowned_env }];
			if method.is_static {
				call_args.push(quote! { class });
			} else {
				call_args.push(quote! { this });
			}
			for param in &method.method_signature.parameters {
				let param_name = &param.name;
				call_args.push(quote! { #param_name });
			}

			quote! {
				#abi_check

				#raw_fn(#(#call_args),*)
			}
		}
		// Direct safe function (fn = path)
		(Some(safe_fn), false) => {
			// Wrap with EnvUnowned::with_env and error handling but skip trait
			let mut call_args = vec![quote! { env }];
			if method.is_static {
				call_args.push(quote! { class });
			} else {
				call_args.push(quote! { this });
			}
			for param in &method.method_signature.parameters {
				let param_name = &param.name;
				call_args.push(quote! { #param_name });
			}

			// Get error policy with fallback chain:
			// 1. method.error_policy if specified
			// 2. default_error_policy if specified
			// 3. abos_jni::errors::ThrowRuntimeExAndDefault
			let error_policy = method
				.error_policy
				.as_ref()
				.or(default_error_policy.as_ref())
				.map_or_else(|| quote! { #jni::errors::ThrowRuntimeExAndDefault }, |p| quote! { #p });

			quote! {
				unowned_env
					.#with_env_api(|env| {
						#abi_check

						#safe_fn(#(#call_args),*)
					})
					.resolve::<#error_policy>()
			}
		}
		// Trait-based raw method (raw = true, no fn)
		(None, true) => {
			// Call trait method directly without with_env wrapper
			let mut call_args = vec![quote! { unowned_env }];
			if method.is_static {
				call_args.push(quote! { class });
			} else {
				call_args.push(quote! { this });
			}
			for param in &method.method_signature.parameters {
				let param_name = &param.name;
				call_args.push(quote! { #param_name });
			}

			quote! {
				#abi_check

				<#api_name as #trait_name>::#rust_name(#(#call_args),*)
			}
		}
		// Trait-based safe method (default: no fn, raw = false)
		(None, false) => {
			// Use trait method with with_env wrapper and error handling
			let mut call_args = vec![quote! { env }];
			if method.is_static {
				call_args.push(quote! { class });
			} else {
				call_args.push(quote! { this });
			}
			for param in &method.method_signature.parameters {
				let param_name = &param.name;
				call_args.push(quote! { #param_name });
			}

			// Get error policy with fallback chain:
			// 1. method.error_policy if specified
			// 2. default_error_policy if specified
			// 3. abos_jni::errors::ThrowRuntimeExAndDefault
			let error_policy = method
				.error_policy
				.as_ref()
				.or(default_error_policy.as_ref())
				.map_or_else(|| quote! { #jni::errors::ThrowRuntimeExAndDefault }, |p| quote! { #p });

			quote! {
				unowned_env
					.#with_env_api(|env| {
						#abi_check

						<#api_name as #trait_name>::#rust_name(#(#call_args),*)
					})
					.resolve::<#error_policy>()
			}
		}
	};

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
