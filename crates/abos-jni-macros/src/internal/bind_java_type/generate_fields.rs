use super::{
	Field, Ident, TokenStream, TypeMappings, extract_cfg_attrs, format_ident, generate_jni_get_field_call,
	generate_jni_set_field_call, quote, sig_type_to_rust_arg_type, sig_type_to_rust_return_type,
};

/// Generate field accessors without their surrounding impl block.
pub(super) fn generate_fields(
	api_name: &Ident,
	fields: &[Field],
	mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let mut accessors: Vec<TokenStream> = Vec::new();
	for field in fields {
		let context = FieldContext::new(field, api_name, mappings, jni);
		if let Some(name) = &field.getter_name {
			accessors.push(context.getter(name));
		}
		if let Some(name) = &field.setter_name {
			accessors.push(context.setter(name));
		}
	}
	quote! { #(#accessors)* }
}

struct FieldContext<'a> {
	field: &'a Field,
	api_name: &'a Ident,
	mappings: &'a TypeMappings,
	jni: &'a syn::Path,
	field_id: TokenStream,
	self_param: TokenStream,
	receiver: TokenStream,
	class_def: TokenStream,
}

impl<'a> FieldContext<'a> {
	fn new(field: &'a Field, api_name: &'a Ident, mappings: &'a TypeMappings, jni: &'a syn::Path) -> Self {
		let id: Ident = format_ident!("{}_field_id", field.rust_name);
		let (self_param, receiver, class_def) = if field.is_static {
			(
				quote! {},
				quote! { class },
				quote! {
					use #jni::refs::Reference as _;
					let class: &#jni::objects::JClass = api.class.as_ref();
				},
			)
		} else {
			(quote! { &self, }, quote! { self }, quote! {})
		};
		Self {
			field,
			api_name,
			mappings,
			jni,
			field_id: quote! { api.#id.into_raw() },
			self_param,
			receiver,
			class_def,
		}
	}

	fn getter(&self, name: &Ident) -> TokenStream {
		let Self {
			field,
			api_name,
			mappings,
			jni,
			field_id,
			self_param,
			receiver,
			class_def,
		} = self;
		let visibility: TokenStream = field.getter_visibility.to_tokens();
		let attributes: Vec<syn::Attribute> = accessor_attributes(field, &field.getter_attrs, true);
		let ty: &crate::internal::types::SigType = &field.signature.field_type;
		let env_type: TokenStream = if ty.try_as_primitive(mappings).is_some() {
			quote! { &#jni::Env<'env_local> }
		} else {
			quote! { &mut #jni::Env<'env_local> }
		};
		let return_type: TokenStream = sig_type_to_rust_return_type(ty, &quote! { 'env_local }, mappings, jni);
		let call: TokenStream =
			generate_jni_get_field_call(&field.java_name, ty, field.is_static, jni, receiver, field_id, mappings);
		let call: TokenStream = super::calls::checked_return(&call, field.non_null, jni);
		quote! {
			#(#attributes)*
			#visibility fn #name<'env_local>(#self_param env: #env_type) -> #jni::errors::Result<#return_type> {
				let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
				#class_def
				#call
			}
		}
	}

	fn setter(&self, name: &Ident) -> TokenStream {
		let Self {
			field,
			api_name,
			mappings,
			jni,
			field_id,
			self_param,
			receiver,
			class_def,
		} = self;
		let visibility: TokenStream = field.setter_visibility.to_tokens();
		let attributes: Vec<syn::Attribute> = accessor_attributes(field, &field.setter_attrs, false);
		let ty: &crate::internal::types::SigType = &field.signature.field_type;
		let lifetime: TokenStream = if ty.try_as_primitive(mappings).is_some() {
			quote! {}
		} else {
			quote! { 'local_field }
		};
		let lifetime_declaration: TokenStream = if lifetime.is_empty() {
			quote! { <'env_local> }
		} else {
			quote! { <'env_local, #lifetime> }
		};
		let arg_type: TokenStream = sig_type_to_rust_arg_type(ty, &lifetime, mappings, jni);
		let call: TokenStream = generate_jni_set_field_call(
			&field.java_name,
			ty,
			field.is_static,
			jni,
			receiver,
			field_id,
			&quote! { val },
			mappings,
		);
		let null_check: TokenStream = if field.non_null {
			quote! {
				use #jni::refs::Reference as _;
				if val.as_ref().is_null() { return Err(#jni::errors::Error::NullPtr("Null Object")); }
			}
		} else {
			quote! {}
		};
		quote! {
			#(#attributes)*
			#visibility fn #name #lifetime_declaration (#self_param env: &#jni::Env<'env_local>, val: #arg_type) -> #jni::errors::Result<()> {
				let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
				#class_def
				#null_check
				#call
			}
		}
	}
}

fn accessor_attributes(field: &Field, explicit: &[syn::Attribute], getter: bool) -> Vec<syn::Attribute> {
	let mut attrs: Vec<syn::Attribute> = if explicit.is_empty() {
		field
			.attrs
			.iter()
			.filter(|attr| getter || !attr.path().is_ident("doc"))
			.cloned()
			.collect()
	} else {
		let mut attrs: Vec<syn::Attribute> = extract_cfg_attrs(&field.attrs);
		attrs.extend_from_slice(explicit);
		attrs
	};
	if !attrs.iter().any(|attr| attr.path().is_ident("doc")) {
		let java_name: &String = &field.java_name;
		let doc: syn::Attribute = if getter {
			syn::parse_quote! { #[doc = concat!("Gets the `", #java_name, "` field.")] }
		} else {
			let getter: &Option<Ident> = &field.getter_name;
			syn::parse_quote! { #[doc = concat!("Sets the `", #java_name, "` field.\n\nSee [`Self::", stringify!(#getter), "`] for more details.")] }
		};
		attrs.insert(0, doc);
	}
	attrs
}
