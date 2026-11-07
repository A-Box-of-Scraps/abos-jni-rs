use super::{ConcreteType, PrimitiveType, Result, SigType, Span, TokenStream, TypeMappings, lit_cstr_mutf8, quote};

/// Helper function to validate that a type can be used as an array element
/// Returns an error if the element type is void
pub(super) fn validate_array_element_type(elem_type: &SigType, type_mappings: &TypeMappings, span: Span) -> Result<()> {
	if let SigType::Alias(name) = elem_type {
		match type_mappings.map_alias(name) {
			Some(ConcreteType::Primitive {
				primitive: PrimitiveType::Void,
				..
			}) => {
				return Err(syn::Error::new(span, "void cannot be used as an array element type"));
			}
			Some(_) => {}
			None => {
				return Err(syn::Error::new(span, format!("Unknown type '{name}'")));
			}
		}
	}

	Ok(())
}

/// Core function to convert a `JavaType` to a concrete Rust type with lifetime
/// Returns the type WITHOUT any `AsRef` wrapper
pub fn sig_type_to_rust_type_core(
	java_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	match java_type {
		SigType::Alias(alias) => match type_mappings.map_alias(alias) {
			Some(ConcreteType::Primitive { primitive, path, .. }) => {
				if primitive == PrimitiveType::Void {
					quote! { () }
				} else {
					let path: syn::Path =
						syn::parse_str(&path).unwrap_or_else(|_| panic!("Invalid Rust type path: {path}"));

					quote! { #path  }
				}
			}
			Some(ConcreteType::Object {
				reference_type: rust_type,
				..
			}) => {
				let path_str: &str = rust_type.path();

				let path: syn::Path =
					syn::parse_str(path_str).unwrap_or_else(|_| panic!("Invalid Rust type path: {path_str}"));

				quote! { #path<#lifetime> }
			}
			None => {
				// Should have been validated while parsing
				unreachable!("Unknown type: {}", alias);
			}
		},
		SigType::Object(class_name) => type_mappings.map_java_class_to_rust_type(class_name).map_or_else(
			|| quote! { #jni::objects::JObject<#lifetime> },
			|rust_type| {
				let path_str: &str = rust_type.path();
				let path: syn::Path =
					syn::parse_str(path_str).unwrap_or_else(|_| panic!("Invalid Rust type path: {path_str}"));
				quote! { #path<#lifetime> }
			},
		),
		SigType::Array(element_type, dimensions) => {
			// Resolve the innermost array before wrapping additional dimensions in JObjectArray.
			let innermost_array: TokenStream = array_element_type(element_type, lifetime, type_mappings, jni);

			let mut result: TokenStream = innermost_array;
			for _ in 0..(*dimensions - 1) {
				result = quote! { #jni::objects::JObjectArray<#lifetime, #result> };
			}
			result
		}
	}
}

fn array_element_type(
	element_type: &SigType,
	lifetime: &TokenStream,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	match element_type {
		SigType::Alias(alias) => {
			match type_mappings.map_alias(alias) {
				Some(ConcreteType::Primitive { primitive, .. }) => {
					if primitive == PrimitiveType::Void {
						unreachable!("void cannot be used as an array element type");
					}
					let prim_ident: proc_macro2::Ident =
						quote::format_ident!("j{}", primitive.jni_name().to_lowercase());
					quote! { #jni::objects::JPrimitiveArray<#lifetime, #jni::sys::#prim_ident> }
				}
				Some(ConcreteType::Object {
					reference_type: rust_type,
					..
				}) => {
					let path_str: &str = rust_type.path();
					let path: syn::Path =
						syn::parse_str(path_str).unwrap_or_else(|_| panic!("Invalid Rust type path: {path_str}"));
					quote! { #jni::objects::JObjectArray<#lifetime, #path<#lifetime>> }
				}
				None => {
					// Should have been validated while parsing
					unreachable!("Unknown type: {}", alias);
				}
			}
		}
		SigType::Object(class_name) => type_mappings.map_java_class_to_rust_type(class_name).map_or_else(
			|| quote! { #jni::objects::JObjectArray<#lifetime> },
			|rust_type| {
				let path_str: &str = rust_type.path();
				let path: syn::Path =
					syn::parse_str(path_str).unwrap_or_else(|_| panic!("Invalid Rust type path: {path_str}"));
				quote! { #jni::objects::JObjectArray<#lifetime, #path<#lifetime>> }
			},
		),
		SigType::Array(_inner_element_type, _dimensions) => {
			// An array of arrays should be represented via `Array(_, dimensions > 1)`, not by nesting
			// ArgType::Array as the element type.
			unreachable!("Multi-dimensional arrays should not be represented by nesting ArgType::Array")
		}
	}
}

/// Generate assertions that type mappings are correct
///
/// - Generates assertions to verify the Java class names match at runtime.
/// - Generates compile-time assertions to verify primitive type sizes and alignments match.
///
/// Skips checks for the built-in jni crate types that are assumed to be correct.
/// (note that we will still generate a compile-time error if attempting to map
/// a built-in Rust type to a different Java type)
pub fn generate_type_mapping_checks(type_mappings: &TypeMappings, jni: &syn::Path) -> TokenStream {
	let mut type_checks: Vec<TokenStream> = Vec::new();

	for (rust_type, java_class) in type_mappings
		.rust_to_java_iter()
		.filter(|(rust_type, _)| !rust_type.is_builtin())
	{
		let rust_path: syn::Path =
			syn::parse_str(rust_type.path()).unwrap_or_else(|_| panic!("Invalid Rust type path: {}", rust_type.path()));

		let java_class_dotted: String = java_class.to_java_dotted();
		let java_class_cstr: syn::LitCStr = lit_cstr_mutf8(&java_class_dotted);

		type_checks.push(quote! {
			{
				// Safety: we have compile-time encoded the name of the Java class as MUTF8
				// and therefore know it's safe to cast as a JNIStr
				let expected_class_name = unsafe {
					#jni::strings::JNIStr::from_cstr_unchecked(#java_class_cstr)
				};
				let actual_class_name = <#rust_path as #jni::refs::Reference>::class_name();
				assert!(
					actual_class_name.as_ref() == expected_class_name,
					"Type mapping mismatch for {}: expected {:?}, got {:?}",
					stringify!(#rust_path),
					expected_class_name,
					actual_class_name
				);
			}
		});
	}

	for rust_type in type_mappings
		.rust_types()
		.filter(|rt| rt.primitive().is_some())
		.filter(|rt| !rt.is_builtin())
	{
		let rust_path: syn::Path =
			syn::parse_str(rust_type.path()).unwrap_or_else(|_| panic!("Invalid Rust type path: {}", rust_type.path()));

		let jni_sys_prim: TokenStream = match rust_type
			.primitive()
			.expect("primitive mappings have a JNI primitive kind")
		{
			PrimitiveType::Void => {
				unreachable!("Void type should not have a custom type mapping")
			}
			PrimitiveType::Boolean => quote! { jboolean },
			PrimitiveType::Byte => quote! { jbyte },
			PrimitiveType::Char => quote! { jchar },
			PrimitiveType::Short => quote! { jshort },
			PrimitiveType::Int => quote! { jint },
			PrimitiveType::Long => quote! { jlong },
			PrimitiveType::Float => quote! { jfloat },
			PrimitiveType::Double => quote! { jdouble },
		};

		type_checks.push(quote! {
            {
                const _: () =  {
                    assert!(::std::mem::size_of::<#jni::sys::#jni_sys_prim>() == ::std::mem::size_of::<#rust_path>(), concat!("Size mismatch for primitive type ", stringify!(#rust_path)));
                    assert!(::std::mem::align_of::<#jni::sys::#jni_sys_prim>() == ::std::mem::align_of::<#rust_path>(), concat!("Alignment mismatch for primitive type ", stringify!(#rust_path)));
                };
            }
        });
	}

	if type_checks.is_empty() {
		quote! {}
	} else {
		quote! {
			#(#type_checks)*
		}
	}
}
