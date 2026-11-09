use super::{
	ConcreteType, PrimitiveType, Result, SigType, Signature, SignatureInput, Span, TokenStream, TypeMappings,
	lit_cstr_mutf8, quote,
};

/// Generate an `abos_jni::signature::JavaType` value for a parsed `SigType`.
pub(super) fn java_type_to_tokens(ty: &SigType, type_mappings: &TypeMappings) -> Result<TokenStream> {
	let jni: &syn::Path = type_mappings.jni_crate();

	match ty {
		SigType::Alias(name) => match type_mappings.map_alias(name) {
			Some(ConcreteType::Object { .. }) => Ok(quote! {
				#jni::signature::JavaType::Object
			}),
			Some(ConcreteType::Primitive { primitive, .. }) => {
				let variant: TokenStream = match primitive {
					PrimitiveType::Boolean => quote! { Boolean },
					PrimitiveType::Byte => quote! { Byte },
					PrimitiveType::Char => quote! { Char },
					PrimitiveType::Short => quote! { Short },
					PrimitiveType::Int => quote! { Int },
					PrimitiveType::Long => quote! { Long },
					PrimitiveType::Float => quote! { Float },
					PrimitiveType::Double => quote! { Double },
					PrimitiveType::Void => quote! { Void },
				};
				Ok(quote! {
					#jni::signature::JavaType::Primitive(#jni::signature::Primitive::#variant)
				})
			}
			None => Err(syn::Error::new(Span::call_site(), format!("Unknown type '{name}'"))),
		},
		SigType::Object(_) => Ok(quote! {
			#jni::signature::JavaType::Object
		}),
		SigType::Array(_, _) => {
			// Array dimensions are encoded in the descriptor, not the `JavaType` variant.
			Ok(quote! {
				#jni::signature::JavaType::Array
			})
		}
	}
}

/// Expand `jni_sig!` into a `MethodSignature` or `FieldSignature` value.
pub fn jni_sig_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	let jni: &syn::Path = type_mappings.jni_crate();

	let jni_sig: String = signature.to_jni_signature(&type_mappings)?;

	let sig_cstr: syn::LitCStr = lit_cstr_mutf8(&jni_sig);

	match signature {
		Signature::Method(method_sig) => {
			let args_tokens: Vec<TokenStream> = method_sig
				.parameters
				.iter()
				.map(|param| java_type_to_tokens(&param.ty, &type_mappings))
				.collect::<Result<Vec<_>>>()?;

			let ret_token: TokenStream = java_type_to_tokens(&method_sig.return_type, &type_mappings)?;

			Ok(quote! {
				{
					// Safety: The signature was parsed and validated at compile-time
					unsafe {
						#jni::signature::MethodSignature::from_raw_parts(
							#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
							&[#(#args_tokens),*],
							#ret_token
						)
					}
				}
			})
		}
		Signature::Field(field_sig) => {
			let ty_token: TokenStream = java_type_to_tokens(&field_sig.field_type, &type_mappings)?;

			Ok(quote! {
				{
					// Safety: The signature was parsed and validated at compile-time
					unsafe {
						#jni::signature::FieldSignature::from_raw_parts(
							#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
							#ty_token
						)
					}
				}
			})
		}
	}
}

/// Expand `jni_sig_str!` into a string literal.
pub fn jni_sig_str_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	let jni_sig: String = signature.to_jni_signature(&type_mappings)?;

	Ok(quote! {
		#jni_sig
	})
}

/// Expand `jni_sig_cstr!` into a modified UTF-8 `CStr` literal.
pub fn jni_sig_cstr_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	let jni_sig: String = signature.to_jni_signature(&type_mappings)?;

	let sig_cstr: syn::LitCStr = lit_cstr_mutf8(&jni_sig);

	Ok(quote! {
		#sig_cstr
	})
}

/// Expand `jni_sig_jstr!` into a modified UTF-8 `&'static JNIStr`.
pub fn jni_sig_jstr_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	let jni: &syn::Path = type_mappings.jni_crate();

	let jni_sig: String = signature.to_jni_signature(&type_mappings)?;

	let sig_cstr: syn::LitCStr = lit_cstr_mutf8(&jni_sig);

	Ok(quote! {
		{
			// Safety: The signature was parsed and validated at compile-time
			unsafe {
				#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr)
			}
		}
	})
}
