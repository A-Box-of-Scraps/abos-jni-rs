use super::*;

/// Convert a JavaType to a TokenStream that constructs abos_jni::signature::JavaType
pub(super) fn java_type_to_tokens(ty: &SigType, type_mappings: &TypeMappings) -> Result<TokenStream> {
	let jni = type_mappings.jni_crate();

	match ty {
		SigType::Alias(name) => match type_mappings.map_alias(name) {
			Some(ConcreteType::Object { .. }) => Ok(quote! {
				#jni::signature::JavaType::Object
			}),
			Some(ConcreteType::Primitive { primitive, .. }) => {
				let variant = match primitive {
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
			None => Err(syn::Error::new(Span::call_site(), format!("Unknown type '{}'", name))),
		},
		SigType::Object(_) => Ok(quote! {
			#jni::signature::JavaType::Object
		}),
		SigType::Array(_, _) => {
			// For arrays, abos_jni::signature::JavaType just uses the Array variant
			// The dimensionality is encoded in the signature string
			Ok(quote! {
				#jni::signature::JavaType::Array
			})
		}
	}
}

/// The actual procedural macro implementation for jni_sig!
/// Returns a abos_jni::signature::{MethodSignature, FieldSignature}
pub fn jni_sig_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	// Get the jni crate path
	let jni = type_mappings.jni_crate();

	// Generate the JNI signature string
	let jni_sig = signature.to_jni_signature(&type_mappings)?;

	// Create a C string literal for the signature with MUTF-8 encoding
	let sig_cstr = lit_cstr_mutf8(&jni_sig);

	// Generate the appropriate abos_jni::signature type
	match signature {
		Signature::Method(method_sig) => {
			// Generate the arguments array
			let args_tokens: Vec<TokenStream> = method_sig
				.parameters
				.iter()
				.map(|param| java_type_to_tokens(&param.ty, &type_mappings))
				.collect::<Result<Vec<_>>>()?;

			// Generate the return type
			let ret_token = java_type_to_tokens(&method_sig.return_type, &type_mappings)?;

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
			// Generate the field type
			let ty_token = java_type_to_tokens(&field_sig.field_type, &type_mappings)?;

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

/// Implementation for jni_sig_str! macro
/// Returns a &str string literal
pub fn jni_sig_str_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	// Generate the JNI signature string
	let jni_sig = signature.to_jni_signature(&type_mappings)?;

	Ok(quote! {
		#jni_sig
	})
}

/// Implementation for jni_sig_cstr! macro
/// Returns a CStr literal (c"...") with MUTF-8 encoding
pub fn jni_sig_cstr_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	// Generate the JNI signature string
	let jni_sig = signature.to_jni_signature(&type_mappings)?;

	// Create a C string literal with MUTF-8 encoding
	let sig_cstr = lit_cstr_mutf8(&jni_sig);

	Ok(quote! {
		#sig_cstr
	})
}

/// Implementation for jni_sig_jstr! macro
/// Returns a &'static JNIStr with MUTF-8 encoding
pub fn jni_sig_jstr_impl(input: TokenStream) -> Result<TokenStream> {
	let SignatureInput {
		signature,
		type_mappings,
	} = syn::parse2(input)?;

	// Get the jni crate path
	let jni = type_mappings.jni_crate();

	// Generate the JNI signature string
	let jni_sig = signature.to_jni_signature(&type_mappings)?;

	// Create a C string literal with MUTF-8 encoding
	let sig_cstr = lit_cstr_mutf8(&jni_sig);

	Ok(quote! {
		{
			// Safety: The signature was parsed and validated at compile-time
			unsafe {
				#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr)
			}
		}
	})
}
