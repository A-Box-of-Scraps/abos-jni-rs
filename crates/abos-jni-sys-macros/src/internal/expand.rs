use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, Ident, Visibility, spanned::Spanned};

use super::version::JniVersion;

fn versioned_fields(input: &DeriveInput) -> syn::Result<Vec<(JniVersion, Field)>> {
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
	fields.named.iter().map(parse_field).collect()
}

fn parse_field(field: &Field) -> syn::Result<(JniVersion, Field)> {
	let mut field: Field = field.clone();
	let mut version: JniVersion = JniVersion::default();
	let mut added = false;
	for attr in &field.attrs {
		if !attr.path().is_ident("jni_added") {
			continue;
		}
		if added {
			return Err(syn::Error::new(attr.span(), "Duplicate jni_added attribute"));
		}
		added = true;
		version = attr.parse_args()?;
	}
	field.attrs.retain(|attr| !attr.path().is_ident("jni_added"));
	Ok((version, field))
}

pub fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
	let original_name: &Ident = &input.ident;
	let visibility: &Visibility = &input.vis;
	let fields: Vec<(JniVersion, Field)> = versioned_fields(input)?;
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
		let version_fields: TokenStream = table_fields(&fields, version);
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

fn table_fields(fields: &[(JniVersion, Field)], version: JniVersion) -> TokenStream {
	// Keep holes within a version's table, but never extend it past its last available slot.
	let end: usize = fields
		.iter()
		.rposition(|(added, _)| *added <= version)
		.map_or(0, |last| last + 1);
	let mut tokens = TokenStream::new();
	let mut padding_index = 0;
	for (added, field) in &fields[..end] {
		if *added > version {
			let padding: Ident = format_ident!("_padding_{padding_index}");
			padding_index += 1;
			tokens.extend(quote! { #padding: *mut ::core::ffi::c_void, });
		} else {
			tokens.extend(quote! { #field, });
		}
	}
	tokens
}
