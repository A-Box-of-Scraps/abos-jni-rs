use crate::internal::types::{native_argument, native_return, native_type};

use super::{
	AbiCheck, Ident, NativeMethod, TokenStream, TypeMappings, extract_cfg_attrs, format_ident,
	generate_native_method_abi_check, quote,
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
	let mut wrappers: Vec<TokenStream> = Vec::new();

	for method in native_methods {
		let wrapper: TokenStream = generate_single_native_wrapper(
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
	let rust_name: &Ident = &method.rust_name;
	let wrapper_name: Ident = format_ident!("{}_native_method", rust_name);

	let lifetime: TokenStream = quote! { 'local };

	let params: Vec<TokenStream> = method.native_parameters(type_name, &lifetime, type_mappings, jni);

	let return_type: TokenStream = native_type(&method.signature.return_type, &lifetime, type_mappings, jni);

	let need_abi_check: AbiCheck = method.abi_check.unwrap_or(default_abi_check);
	let abi_check: TokenStream = generate_native_method_abi_check(
		jni,
		&method.java_name,
		need_abi_check,
		method.is_raw,
		method.is_static,
		None, // type mappings are checked in the TypeAPI::get() method
	);

	let with_env_api: TokenStream = if method.catch_unwind.unwrap_or(default_native_methods_catch_unwind) {
		quote! { with_env }
	} else {
		quote! { with_env_no_catch }
	};

	let wrapper_body: TokenStream = wrapper_body(
		method,
		api_name,
		trait_name,
		&abi_check,
		&with_env_api,
		default_error_policy,
		jni,
	);

	let conversions: Vec<TokenStream> = method
		.signature
		.parameters
		.iter()
		.filter(|parameter| crate::internal::types::is_rust_bool(&parameter.ty, type_mappings))
		.map(|parameter| {
			let name: &Ident = &parameter.name;
			let value: TokenStream = native_argument(&parameter.ty, name, type_mappings, jni);
			quote! { let #name: bool = #value; }
		})
		.collect();
	let wrapper_body: TokenStream = native_return(&method.signature.return_type, wrapper_body, type_mappings, jni);
	let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&method.attrs);

	quote! {
		#(#cfg_attrs)*
		#[allow(unused)]
		extern "system" fn #wrapper_name<#lifetime>(
			#(#params),*
		) -> #return_type {
			#(#conversions)*
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
	let rust_name: &Ident = &method.rust_name;
	let callable: TokenStream = method.native_fn.as_ref().map_or_else(
		|| quote! { <#api_name as #trait_name>::#rust_name },
		|path| quote! { #path },
	);
	let environment: TokenStream = if method.is_raw {
		quote! { unowned_env }
	} else {
		quote! { env }
	};
	let receiver: TokenStream = if method.is_static {
		quote! { class }
	} else {
		quote! { this }
	};
	let parameters: Vec<&Ident> = method.signature.parameters.iter().map(|param| &param.name).collect();
	let call: TokenStream = quote! { #callable(#environment, #receiver, #(#parameters),*) };
	if method.is_raw {
		return quote! { #abi_check #call };
	}
	let error_policy: TokenStream = method.error_policy.as_ref().or(default_error_policy).map_or_else(
		|| quote! { #jni::errors::ThrowRuntimeExAndDefault },
		|policy| quote! { #policy },
	);
	quote! {
		unowned_env.#with_env_api(|env| { #abi_check #call }).resolve::<#error_policy>()
	}
}

impl NativeMethod {
	pub(super) fn native_parameters(
		&self,
		type_name: &Ident,
		lifetime: &TokenStream,
		type_mappings: &TypeMappings,
		jni: &syn::Path,
	) -> Vec<TokenStream> {
		let mut params: Vec<TokenStream> = Vec::new();
		params.push(quote! { mut unowned_env: #jni::EnvUnowned<#lifetime> });

		if self.is_static {
			params.push(quote! { class: #jni::objects::JClass<#lifetime> });
		} else {
			params.push(quote! { this: #type_name<#lifetime> });
		}

		// The wrapper uses ABI types; bool conversion happens before calling the trait.
		for param in &self.signature.parameters {
			let param_name: &Ident = &param.name;
			let rust_type: TokenStream = native_type(&param.ty, lifetime, type_mappings, jni);
			params.push(quote! { #param_name: #rust_type });
		}

		params
	}
}
