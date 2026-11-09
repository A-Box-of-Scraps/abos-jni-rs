use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, Ident, Visibility, spanned::Spanned};

use super::version::JniVersion;

fn versioned_fields(
	input: &DeriveInput,
	attribute: &str,
	default: JniVersion,
) -> syn::Result<Vec<(JniVersion, Field)>> {
	if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
		return Err(syn::Error::new(
			input.generics.span(),
			"Generic JNI tables are not supported",
		));
	}
	let Data::Struct(data) = &input.data else {
		return Err(syn::Error::new(input.span(), "Expected a struct with named fields"));
	};
	let Fields::Named(fields) = &data.fields else {
		return Err(syn::Error::new(input.span(), "Expected a struct with named fields"));
	};
	if fields.named.is_empty() {
		return Err(syn::Error::new(input.span(), "Expected at least one JNI table field"));
	}
	fields
		.named
		.iter()
		.map(|field| parse_field(field, attribute, default))
		.collect()
}

fn parse_field(field: &Field, attribute: &str, default: JniVersion) -> syn::Result<(JniVersion, Field)> {
	let mut field: Field = field.clone();
	let mut version: JniVersion = default;
	let mut added = false;
	for attr in &field.attrs {
		if !attr.path().is_ident(attribute) {
			continue;
		}
		if added {
			return Err(syn::Error::new(attr.span(), format!("Duplicate {attribute} attribute")));
		}
		added = true;
		version = attr.parse_args()?;
	}
	field.attrs.retain(|attr| !attr.path().is_ident(attribute));
	Ok((version, field))
}

pub fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
	let original_name: &Ident = &input.ident;
	let visibility: &Visibility = &input.vis;
	let fields: Vec<(JniVersion, Field)> = versioned_fields(input, "jni_added", JniVersion::default())?;
	let versions: BTreeSet<JniVersion> = fields.iter().map(|(version, _)| *version).collect();
	let mut expanded = TokenStream::new();
	let mut members = TokenStream::new();

	for version in versions {
		let suffix: String = version.suffix();
		let struct_name: Ident = format_ident!("{original_name}_{suffix}");
		let member_name: Ident = if version == JniVersion::Reserved {
			format_ident!("reserved")
		} else {
			format_ident!("v{suffix}")
		};
		let version_fields: TokenStream = table_fields(&fields, version, false);
		expanded.extend(quote! {
			#[allow(non_snake_case, non_camel_case_types, reason = "Names match the JNI API and its versions.")]
			#[repr(C)]
			#[derive(Copy, Clone)]
			#visibility struct #struct_name {
				#version_fields
			}
		});
		let api_comment: String = format!("API when JNI version >= `JNI_VERSION_{suffix}`");
		members.extend(quote! {
			#[doc = #api_comment]
			#visibility #member_name: #struct_name,
		});
	}

	expanded.extend(quote! {
		#[repr(C)]
		#visibility union #original_name {
			#members
		}
	});
	Ok(expanded)
}

fn table_fields(fields: &[(JniVersion, Field)], version: JniVersion, public_padding: bool) -> TokenStream {
	// Keep holes within a version's table, but never extend it past its last available slot.
	let end: usize = fields
		.iter()
		.rposition(|(added, _)| *added <= version)
		.map_or(0, |last| last + 1);
	let mut tokens = TokenStream::new();
	let mut padding_index = 0;
	for (index, (added, field)) in fields[..end].iter().enumerate() {
		if *added > version {
			if public_padding {
				let padding: Ident = format_ident!("reserved{}", index + 1);
				tokens.extend(quote! { pub #padding: *mut ::core::ffi::c_void, });
			} else {
				let padding: Ident = format_ident!("_padding_{padding_index}");
				padding_index += 1;
				tokens.extend(quote! { #padding: *mut ::core::ffi::c_void, });
			}
		} else {
			tokens.extend(quote! { #field, });
		}
	}
	tokens
}

/// Retain the full raw JVMTI table and add bounded historical views.
pub fn expand_jvmti(input: &DeriveInput) -> syn::Result<TokenStream> {
	let baseline: JniVersion = JniVersion::Number { major: 1, minor: 2 };
	let fields: Vec<(JniVersion, Field)> = versioned_fields(input, "jvmti_added", baseline)?;
	let versions: BTreeSet<JniVersion> = fields
		.iter()
		.map(|(version, _)| *version)
		.filter(|version| *version != JniVersion::Reserved)
		.collect();
	let name: &Ident = &input.ident;
	let visibility: &Visibility = &input.vis;
	let attributes: &[syn::Attribute] = &input.attrs;
	let full_fields: Vec<&Field> = fields.iter().map(|(_, field)| field).collect();
	let mut expanded: TokenStream = quote! {
		#(#attributes)*
		#visibility struct #name { #(#full_fields,)* }
	};
	for version in versions {
		let view: Ident = format_ident!("{name}_{}", version.suffix());
		let view_fields: TokenStream = table_fields(&fields, version, true);
		let documentation: String = format!(
			"Function-table view for JVMTI {} or newer. Reserved slots must not be called.",
			version.suffix()
		);
		expanded.extend(quote! {
			#[doc = #documentation]
			#[repr(C)]
			#[derive(Clone, Copy, Debug, Default)]
			#visibility struct #view { #view_fields }
		});
	}
	Ok(expanded)
}
