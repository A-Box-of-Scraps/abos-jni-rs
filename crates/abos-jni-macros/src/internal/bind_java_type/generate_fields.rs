use super::{
	Field, Ident, TokenStream, TypeMappings, extract_cfg_attrs, format_ident, generate_jni_get_field_call,
	generate_jni_set_field_call, quote, sig_type_to_rust_arg_type, sig_type_to_rust_return_type,
};

/// Generate field getter/setter implementations (without any impl block wrapper)
pub(super) fn generate_fields(
	api_name: &Ident,
	fields: &[Field],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let mut field_impls = Vec::new();

	for field in fields {
		let rust_name = &field.rust_name;
		let field_id_field = format_ident!("{}_field_id", rust_name);

		// Determine environment type based on field type (primitives use &Env, objects use &mut Env)
		let get_env_type = if field
			.field_signature
			.field_type
			.try_as_primitive(type_mappings)
			.is_some()
		{
			quote! { &#jni::Env<'env_local> }
		} else {
			quote! { &mut #jni::Env<'env_local> }
		};

		// Setters always use &Env (non-mut)
		let set_env_type = quote! { &#jni::Env<'env_local> };

		// Generate return type
		let return_type = sig_type_to_rust_return_type(
			&field.field_signature.field_type,
			&quote! { 'env_local },
			type_mappings,
			jni,
		);

		// Generate the field type lifetime if needed
		let field_lifetime = if field
			.field_signature
			.field_type
			.try_as_primitive(type_mappings)
			.is_some()
		{
			quote! {}
		} else {
			quote! { 'local_field }
		};

		let field_lifetime_decl = if field_lifetime.is_empty() {
			quote! { <'env_local> }
		} else {
			quote! { <'env_local, #field_lifetime> }
		};

		// Generate argument type for setter
		let arg_type =
			sig_type_to_rust_arg_type(&field.field_signature.field_type, &field_lifetime, type_mappings, jni);

		let self_param = if field.is_static {
			quote! {}
		} else {
			quote! { &self, }
		};

		// Generate the this_or_class expression
		let this_or_class = if field.is_static {
			quote! { class }
		} else {
			quote! { self }
		};

		let class_def = if field.is_static {
			quote! {
				use #jni::refs::Reference as _;
				let class: &#jni::objects::JClass = api.class.as_ref();
			}
		} else {
			quote! {}
		};

		if let Some(getter_name) = &field.getter_name {
			let getter_visibility = field.getter_visibility.to_tokens();

			// Always start with cfg attributes from the field itself
			let field_cfg_attrs = extract_cfg_attrs(&field.attrs);

			// Determine which attributes to apply to getter
			// If getter_attrs are explicitly set, use those (plus field cfg)
			// Otherwise, use the field's attrs
			let mut getter_attributes = if field.getter_attrs.is_empty() {
				field.attrs.clone()
			} else {
				// Merge: field cfg attrs + explicit getter attrs
				let mut attrs = field_cfg_attrs.clone();
				attrs.extend(field.getter_attrs.clone());
				attrs
			};

			if !getter_attributes.iter().any(|attr| attr.path().is_ident("doc")) {
				// If no doc attribute, add a default one
				let java_name = &field.java_name;
				let doc_attr: syn::Attribute = syn::parse_quote! {
					#[doc = concat!("Gets the `", #java_name, "` field.")]
				};
				getter_attributes.insert(0, doc_attr);
			}

			// Generate getter
			let get_call = generate_jni_get_field_call(
				&field.java_name,
				&field.field_signature.field_type,
				field.is_static,
				jni,
				&this_or_class,
				&quote! { api.#field_id_field.into_raw() },
				type_mappings,
			);

			// Generate null check if non_null is true
			let get_null_check_and_return = if field.non_null {
				quote! {
					#get_call.and_then(|__result| {
						use #jni::refs::Reference as _;
						if __result.is_null() {
							Err(#jni::errors::Error::NullPtr("Null Object"))
						} else {
							Ok(__result)
						}
					})
				}
			} else {
				quote! {
					#get_call
				}
			};

			field_impls.push(quote! {
				#(#getter_attributes)*
				#getter_visibility fn #getter_name<'env_local>(
					#self_param
					env: #get_env_type
				) -> #jni::errors::Result<#return_type> {
					let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
					#class_def
					#get_null_check_and_return
				}
			});
		}

		if let Some(setter_name) = &field.setter_name {
			let setter_visibility = field.setter_visibility.to_tokens();

			// Always start with cfg attributes from the field itself
			let field_cfg_attrs = extract_cfg_attrs(&field.attrs);

			// Determine which attributes to apply to setter
			// If setter_attrs are explicitly set, use those (plus field cfg)
			// Otherwise, use the field's attrs (excluding doc)
			let mut setter_attributes: Vec<syn::Attribute> = if field.setter_attrs.is_empty() {
				field
					.attrs
					.iter()
					.filter(|attr| !attr.path().is_ident("doc"))
					.cloned()
					.collect()
			} else {
				// Merge: field cfg attrs + explicit setter attrs
				let mut attrs = field_cfg_attrs.clone();
				attrs.extend(field.setter_attrs.clone());
				attrs
			};

			if !setter_attributes.iter().any(|attr| attr.path().is_ident("doc")) {
				// If no doc attribute, add a default one
				let java_name = &field.java_name;
				let getter = &field.getter_name;
				let doc_attr: syn::Attribute = syn::parse_quote! {
					#[doc = concat!("Sets the `", #java_name, "` field.\n\nSee [`Self::", stringify!(#getter), "`] for more details.")]
				};
				setter_attributes.insert(0, doc_attr);
			}

			// Generate setter
			let set_call = generate_jni_set_field_call(
				&field.java_name,
				&field.field_signature.field_type,
				field.is_static,
				jni,
				&this_or_class,
				&quote! { api.#field_id_field.into_raw() },
				&quote! { val },
				type_mappings,
			);

			// Generate null check if non_null is true
			let set_null_check_and_call = if field.non_null {
				quote! {
					use #jni::refs::Reference as _;
					if val.as_ref().is_null() {
						return Err(#jni::errors::Error::NullPtr("Null Object"));
					}
					#set_call
				}
			} else {
				quote! {
					#set_call
				}
			};

			field_impls.push(quote! {
				#(#setter_attributes)*
				#setter_visibility fn #setter_name #field_lifetime_decl (
					#self_param
					env: #set_env_type,
					val: #arg_type
				) -> #jni::errors::Result<()> {
					let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
					#class_def
					#set_null_check_and_call
				}
			});
		}
	}

	// Return just the field implementations without any impl block wrapper
	quote! {
		#(#field_impls)*
	}
}
