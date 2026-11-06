use super::{
	Field, IdLookup, Method, Result, TokenStream, TypeMappings, extract_cfg_attrs, format_ident,
	generate_id_fields_and_inits, quote,
};

/// Generate method ID fields and initialization code for methods
pub(super) fn generate_method_ids(
	methods: &[Method],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<(Vec<TokenStream>, Vec<TokenStream>)> {
	let mut lookups: Vec<IdLookup> = Vec::new();

	for method in methods {
		let rust_name: &proc_macro2::Ident = &method.rust_name;
		let java_name: &String = &method.java_name;
		let method_id_field: proc_macro2::Ident = format_ident!("{}_method_id", rust_name);
		let is_static: bool = method.is_static;

		// Use the MethodSignature to generate JNI signature
		let jni_sig_str: String = method.signature.to_jni_signature(type_mappings).map_err(|e| {
			syn::Error::new(
				rust_name.span(),
				format!("Failed to generate JNI signature for method '{rust_name}': {e}"),
			)
		})?;

		let lookup_fn: proc_macro2::Ident = if is_static {
			format_ident!("get_static_method_id")
		} else {
			format_ident!("get_method_id")
		};

		let field_type: TokenStream = if is_static {
			quote! { #jni::ids::JStaticMethodID }
		} else {
			quote! { #jni::ids::JMethodID }
		};

		let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&method.attrs);

		lookups.push(IdLookup {
			field_name: method_id_field,
			field_type,
			java_name: java_name.clone(),
			signature: jni_sig_str,
			lookup_fn,
			cfg_attrs,
		});
	}

	Ok(generate_id_fields_and_inits(lookups, jni))
}

/// Generate field ID fields and initialization code for fields
pub(super) fn generate_field_ids(
	fields: &[Field],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<(Vec<TokenStream>, Vec<TokenStream>)> {
	let mut lookups: Vec<IdLookup> = Vec::new();

	for field in fields {
		let rust_name: &proc_macro2::Ident = &field.rust_name;
		let java_name: &String = &field.java_name;
		let field_id_field: proc_macro2::Ident = format_ident!("{}_field_id", rust_name);

		// Use the FieldSignature to generate JNI signature
		let field_sig: String = field.signature.to_jni_signature(type_mappings).map_err(|e| {
			syn::Error::new(
				rust_name.span(),
				format!("Failed to generate JNI signature for field '{rust_name}': {e}"),
			)
		})?;

		let lookup_fn: proc_macro2::Ident = if field.is_static {
			format_ident!("get_static_field_id")
		} else {
			format_ident!("get_field_id")
		};

		let field_type: TokenStream = if field.is_static {
			quote! { #jni::ids::JStaticFieldID }
		} else {
			quote! { #jni::ids::JFieldID }
		};

		let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&field.attrs);

		lookups.push(IdLookup {
			field_name: field_id_field,
			field_type,
			java_name: java_name.clone(),
			signature: field_sig,
			lookup_fn,
			cfg_attrs,
		});
	}

	Ok(generate_id_fields_and_inits(lookups, jni))
}
