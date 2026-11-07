use super::{
	AbiCheck, Ident, IsInstanceOfEntry, Result, TokenStream, TypeMappings, VisibilitySpec, format_ident,
	generate_is_instance_of_assignable_checks, generate_type_mapping_checks, path_to_string_no_spaces, quote,
};

/// Generate the type struct definition
pub(super) fn generate_type_struct(
	type_name: &Ident,
	type_visibility: &VisibilitySpec,
	type_attrs: &[syn::Attribute],
	java_class: &str,
	jni: &syn::Path,
) -> TokenStream {
	let has_doc_attr: bool = type_attrs.iter().any(|attr| attr.path().is_ident("doc"));

	let jni_path_str: String = path_to_string_no_spaces(jni);

	let doc_attr: TokenStream = if has_doc_attr {
		quote! {}
	} else {
		quote! {
			#[doc = concat!(
				r#"A `"#, #java_class, r#"` reference, tied to a JNI local reference frame.

See the [`JObject`] documentation for more information about object references,
how to cast them, and local reference frame lifetimes.

[`JObject`]: "#, #jni_path_str, r#"::objects::JObject
"#
			)]
		}
	};

	let type_visibility: TokenStream = type_visibility.to_tokens();

	quote! {
		#(#type_attrs)*
		#doc_attr
		#[repr(transparent)]
		#[derive(Debug, Default)]
		#type_visibility struct #type_name<'local>(#jni::objects::JObject<'local>);

		impl<'local> ::core::convert::AsRef<#type_name<'local>> for #type_name<'local> {
			#[inline]
			fn as_ref(&self) -> &#type_name<'local> {
				self
			}
		}

		impl<'local> ::core::convert::AsRef<#jni::objects::JObject<'local>> for #type_name<'local> {
			#[inline]
			fn as_ref(&self) -> &#jni::objects::JObject<'local> {
				self
			}
		}

		impl<'local> ::core::ops::Deref for #type_name<'local> {
			type Target = #jni::objects::JObject<'local>;

			#[inline]
			fn deref(&self) -> &Self::Target {
				&self.0
			}
		}

		impl<'local> ::core::convert::From<#type_name<'local>> for #jni::objects::JObject<'local> {
			#[inline]
			fn from(other: #type_name<'local>) -> #jni::objects::JObject<'local> {
				other.0
			}
		}
	}
}

/// Generate the API struct definition
pub(super) fn generate_api_struct(
	api_name: &Ident,
	type_visibility: &VisibilitySpec,
	priv_type: Option<&Ident>,
	method_id_fields: &[TokenStream],
	jni: &syn::Path,
) -> TokenStream {
	let priv_field: TokenStream = priv_type.map_or_else(|| quote! {}, |priv_ty| quote! { private: #priv_ty, });

	let api_visibility: TokenStream = VisibilitySpec::PubCrate.clamp_to(type_visibility).to_tokens();
	quote! {
		#[allow(non_snake_case)]
		#api_visibility struct #api_name {
			class: #jni::refs::Global<#jni::objects::JClass<'static>>,
			#priv_field
			#(#method_id_fields)*
		}

		unsafe impl ::core::marker::Send for #api_name {}
		unsafe impl ::core::marker::Sync for #api_name {}
	}
}

/// Generate the `API::get()` method
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_api_get_method(
	api_name: &Ident,
	type_name: &Ident,
	java_class: &str,
	load_class_closure: Option<&TokenStream>,
	init_priv_closure: Option<&TokenStream>,
	priv_type: Option<&Ident>,
	method_id_inits: &[TokenStream],
	native_registration_code: &TokenStream,
	is_instance_of: &[IsInstanceOfEntry],
	type_mappings: &TypeMappings,
	abi_check: AbiCheck,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let load_class_wrapper_name: Ident = format_ident!("__{}_load_class_wrapper", type_name.to_string().to_lowercase());
	let init_priv_wrapper_name: Ident = format_ident!("__{}_init_priv_wrapper", type_name.to_string().to_lowercase());

	let load_class_wrapper: TokenStream = if load_class_closure.is_some() {
		load_class_hook_wrapper(&load_class_wrapper_name, jni)
	} else {
		quote! {}
	};

	let init_priv_wrapper: TokenStream = match (priv_type, init_priv_closure) {
		(Some(priv_type), Some(_)) => init_priv_hook_wrapper(&init_priv_wrapper_name, priv_type, jni),
		_ => quote! {},
	};

	let load_class_code: TokenStream = load_class_closure.map_or_else(
		|| {
			quote! {
				loader.load_class_for_type::<#type_name>(env, false)?
			}
		},
		|closure| {
			quote! {
				#load_class_wrapper_name(env, loader, false, #closure)?
			}
		},
	);

	let init_priv_code: TokenStream = init_priv_closure.map_or_else(
		|| quote! {},
		|closure| {
			quote! {
				let private = #init_priv_wrapper_name(env, loader, class, #closure)?;
			}
		},
	);

	let is_instance_of_checks: TokenStream =
		generate_is_instance_of_assignable_checks(type_name, java_class, is_instance_of, type_mappings, jni)?;

	let type_mapping_runtime_checks: TokenStream = if abi_check.requires_abi_check() {
		generate_type_mapping_checks(type_mappings, jni)
	} else {
		quote! {}
	};

	let private_field: Option<TokenStream> = priv_type.map(|_| quote! { private, });
	let api_construction: TokenStream = quote! {
		// Compile-time encoded modified UTF-8 names are valid JNIStr values.
		unsafe { Self { class: env.new_global_ref(class)?, #private_field #(#method_id_inits)* } }
	};

	Ok(quote! {
		#load_class_wrapper
		#init_priv_wrapper

		impl #api_name {
			pub fn get(
				env: &#jni::Env,
				loader: &#jni::refs::LoaderContext,
			) -> #jni::errors::Result<&'static Self> {
				static API: ::std::sync::OnceLock<#api_name> = ::std::sync::OnceLock::new();

				if let Some(api) = API.get() {
					return Ok(api);
				}

				// Class initialization can call back into Rust, so lookups and API initialization
				// must run without holding a lock. Concurrent callers may repeat this work.

				let api = env.with_local_frame(4, |env| -> #jni::errors::Result<#api_name> {
					// Cache the class before initialization can re-enter get().
					static CLASS: ::std::sync::OnceLock<#jni::objects::Global<#jni::objects::JClass>> = ::std::sync::OnceLock::new();

					let class = if let Some(class) = CLASS.get() {
						class
					} else {
						let class: #jni::objects::JClass = {
							#load_class_code
						};
						let global_class = env.new_global_ref(&class)?;
						let _ = CLASS.set(global_class);
						CLASS.get().unwrap()
					};
					let class: &#jni::objects::JClass = class.as_ref();

					// Register natives before class initialization can call them.
					#native_registration_code

					#init_priv_code

					#is_instance_of_checks

					#type_mapping_runtime_checks

					let api = #api_construction;

					Ok(api)
				})?;
				let _ = API.set(api);
				Ok(API.get().unwrap())
			}
		}
	})
}

fn load_class_hook_wrapper(load_class_wrapper_name: &Ident, jni: &syn::Path) -> TokenStream {
	quote! {
		#[allow(unused, non_snake_case)]
		fn #load_class_wrapper_name<'env_local, F>(
			env: &mut #jni::Env<'env_local>,
			loader: &#jni::refs::LoaderContext,
			initialize: bool,
			load_class: F,
		) -> #jni::errors::Result<#jni::objects::JClass<'env_local>>
		where
			F: FnOnce(
				&mut #jni::Env<'env_local>,
				&#jni::refs::LoaderContext,
				bool,
			) -> #jni::errors::Result<#jni::objects::JClass<'env_local>>,
		{
			load_class(env, loader, initialize)
		}
	}
}

fn init_priv_hook_wrapper(init_priv_wrapper_name: &Ident, priv_type: &Ident, jni: &syn::Path) -> TokenStream {
	quote! {
		#[allow(unused, non_snake_case)]
		fn #init_priv_wrapper_name<'env_local, F>(
			env: &mut #jni::Env<'env_local>,
			loader: &#jni::refs::LoaderContext,
			class: &#jni::objects::JClass,
			init_priv: F,
		) -> #jni::errors::Result<#priv_type>
		where
			F: FnOnce(
				&mut #jni::Env<'env_local>,
				&#jni::refs::LoaderContext,
				&#jni::objects::JClass,
			) -> #jni::errors::Result<#priv_type>,
			#priv_type: Send + Sync
		{
			init_priv(env, loader, class)
		}
	}
}
