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
	// Check if there's a doc attribute in type_attrs
	let has_doc_attr: bool = type_attrs.iter().any(|attr| attr.path().is_ident("doc"));

	let jni_path_str: String = path_to_string_no_spaces(jni);

	// Generate default doc comment if no doc attribute provided
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
	// Generate wrapper function names to avoid conflicts
	let load_class_wrapper_name: Ident = format_ident!("__{}_load_class_wrapper", type_name.to_string().to_lowercase());
	let init_priv_wrapper_name: Ident = format_ident!("__{}_init_priv_wrapper", type_name.to_string().to_lowercase());

	// Generate wrapper functions (outside the impl block)
	let load_class_wrapper: TokenStream = if load_class_closure.is_some() {
		load_class_hook_wrapper(&load_class_wrapper_name, jni)
	} else {
		quote! {}
	};

	let init_priv_wrapper: TokenStream = match (priv_type, init_priv_closure) {
		(Some(priv_type), Some(_)) => init_priv_hook_wrapper(&init_priv_wrapper_name, priv_type, jni),
		_ => quote! {},
	};

	// Generate the class loading code
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

	// Generate the priv initialization code
	let init_priv_code: TokenStream = init_priv_closure.map_or_else(
		|| quote! {},
		|closure| {
			quote! {
				let private = #init_priv_wrapper_name(env, loader, class, #closure)?;
			}
		},
	);

	// Generate IsAssignable checks for all is_instance_of types
	let is_instance_of_checks: TokenStream =
		generate_is_instance_of_assignable_checks(type_name, java_class, is_instance_of, type_mappings, jni)?;

	// Generate type mapping assertions
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

				// Fast path: already initialized
				if let Some(api) = API.get() {
					return Ok(api);
				}

				// Slow path: Lookup class

				// The general pattern here is to avoid holding any lock while
				// performing class lookups and API initialization in case we need
				// to be re-entrant (e.g. due to class initializers that call back
				// into Rust).
				//
				// This matters for types where we lookup method IDs and field IDs
				// which may trigger class initialization, and especially for types
				// that register native methods that may need to be registered
				// before the class can be initialized and then called during class
				// initialization.

				// NB: the purpose of the `OnceLock` here is to amortize the cost of
				// class lookups and API initialization over multiple uses, so we aren't
				// really concerned about a small amount of redundant work if multiple
				// threads race here.

				let api = env.with_local_frame(4, |env| -> #jni::errors::Result<#api_name> {
					// We cache the class early to avoid repeat lookups just in case
					// `::get()` is re-entered by the same thread during class
					// initialization.
					//
					// After `API` is set this `CLASS` cache won't be used again.
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

					// Register native methods ASAP, before anything could
					// trigger class initialization that might itself need to
					// call native methods.
					#native_registration_code

					#init_priv_code

					// Assert that all declared is_instance_of types are valid
					#is_instance_of_checks

					// Assert that all type mappings are correct
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
