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
	let has_doc_attr = type_attrs.iter().any(|attr| attr.path().is_ident("doc"));

	let jni_path_str = path_to_string_no_spaces(jni);

	// Generate default doc comment if no doc attribute provided
	let doc_attr = if has_doc_attr {
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

	let type_visibility = type_visibility.to_tokens();

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
	let priv_field = if let Some(priv_ty) = priv_type {
		quote! { private: #priv_ty, }
	} else {
		quote! {}
	};

	let api_visibility = VisibilitySpec::PubCrate.clamp_to(type_visibility).to_tokens();
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
#[allow(clippy::too_many_arguments)]
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
	let load_class_wrapper_name = format_ident!("__{}_load_class_wrapper", type_name.to_string().to_lowercase());
	let init_priv_wrapper_name = format_ident!("__{}_init_priv_wrapper", type_name.to_string().to_lowercase());

	// Generate wrapper functions (outside the impl block)
	let load_class_wrapper = if load_class_closure.is_some() {
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
	} else {
		quote! {}
	};

	#[allow(clippy::collapsible_if)]
	let init_priv_wrapper = if let Some(priv_type) = priv_type {
		if init_priv_closure.is_some() {
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
		} else {
			quote! {}
		}
	} else {
		quote! {}
	};

	// Generate the class loading code
	let load_class_code = if let Some(closure) = load_class_closure {
		quote! {
			#load_class_wrapper_name(env, loader, false, #closure)?
		}
	} else {
		quote! {
			loader.load_class_for_type::<#type_name>(env, false)?
		}
	};

	// Generate the priv initialization code
	let init_priv_code = if let Some(closure) = init_priv_closure {
		quote! {
			let private = #init_priv_wrapper_name(env, loader, class, #closure)?;
		}
	} else {
		quote! {}
	};

	// Generate IsAssignable checks for all is_instance_of types
	let is_instance_of_checks =
		generate_is_instance_of_assignable_checks(type_name, java_class, is_instance_of, type_mappings, jni)?;

	// Generate type mapping assertions
	let type_mapping_runtime_checks = if abi_check.requires_abi_check() {
		generate_type_mapping_checks(type_mappings, jni)
	} else {
		quote! {}
	};

	let api_construction = if priv_type.is_some() {
		quote! {
			// All of the CStr literals have been validated at compile time and
			// since they have been encoded as MUTF-8 they can be safely cast as
			// a JNIStr without a runtime check.
			unsafe {
				Self {
					class: env.new_global_ref(class)?,
					private,
					#(#method_id_inits)*
				}
			}
		}
	} else {
		quote! {
			// All of the CStr literals have been validated at compile time and
			// since they have been encoded as MUTF-8 they can be safely cast as
			// a JNIStr without a runtime check.
			unsafe {
				Self {
					class: env.new_global_ref(class)?,
					#(#method_id_inits)*
				}
			}
		}
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
