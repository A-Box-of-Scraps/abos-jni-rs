use super::{
	ConcreteType, Ident, IsInstanceOfEntry, Result, Span, TokenStream, TypeMappings, format_ident, lit_cstr_mutf8,
	path_to_string_no_spaces, quote,
};

/// Generate `IsAssignable` checks for all declared `is_instance_of` types
pub(super) fn generate_is_instance_of_assignable_checks(
	type_name: &Ident,
	java_class: &str,
	is_instance_of: &[IsInstanceOfEntry],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let checks: Vec<_> = is_instance_of
		.iter()
		.map(|entry| {
			let to_rust_type_alias = &entry.type_alias;

			match type_mappings.map_alias(to_rust_type_alias) {
				Some(ConcreteType::Primitive { .. }) => Err(syn::Error::new(
					Span::call_site(),
					format!("Primitive type '{to_rust_type_alias}' cannot be used in is_instance_of"),
				)),
				Some(ConcreteType::Object {
					name: to_java_class,
					reference_type: to_rust_type,
				}) => {
					let to_type_path_str = &to_rust_type.path();

					let to_java_class_str = to_java_class.to_java_dotted();

					let to_type_path: syn::Path = syn::parse_str(to_type_path_str).map_err(|_| {
						syn::Error::new(Span::call_site(), format!("Invalid Rust type path: {to_type_path_str}"))
					})?;

					Ok(quote! {
						{
							let is_instance_of_class = <#to_type_path as #jni::refs::Reference>::lookup_class(env, loader)?;
							let is_instance_of_class: &#jni::objects::JClass = unsafe { is_instance_of_class.as_ref() };

							// IsAssignableFrom is not exception safe, so we can't go ahead with the JNI call if
							// there's currently a pending exception.
							if env.exception_check() {
								return Err(#jni::errors::Error::JavaException);
							}
							// Call IsAssignableFrom to check if is_instance_of_class is assignable from our class
							let is_assignable = unsafe {
								use #jni::refs::Reference as _;
								let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
								let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
								((*interface).v1_1.IsAssignableFrom)(
									env_ptr,
									class.as_raw(), // From
									is_instance_of_class.as_raw(), // To
								)
							};

							assert!(
								is_assignable,
								"{}({}) is not a subtype of {}({}) and is not a valid 'is_instance_of' type for {}",
								#to_type_path_str,
								#to_java_class_str,
								stringify!(#type_name),
								#java_class,
								stringify!(#type_name)
							);
						}
					})
				}
				None => Err(syn::Error::new(
					Span::call_site(),
					format!("Type '{to_rust_type_alias}' used in is_instance_of is not defined in types mapping"),
				)),
			}
		})
		.collect::<Result<Vec<_>>>()?;

	if checks.is_empty() {
		Ok(quote! {})
	} else {
		Ok(quote! {
			#(#checks)*
		})
	}
}

/// Generate Reference trait implementation
pub(super) fn generate_reference_impl(
	type_name: &Ident,
	api_name: &Ident,
	java_class: &str,
	jni: &syn::Path,
) -> TokenStream {
	// Create a CStr literal for the Java class name
	let java_class_cstr = lit_cstr_mutf8(java_class);

	quote! {
		unsafe impl<'local> #jni::refs::Reference for #type_name<'local> {
			type Kind<'env> = #type_name<'env>;
			type GlobalKind = #type_name<'static>;

			#[inline]
			fn as_raw(&self) -> #jni::sys::jobject {
				self.0.as_raw()
			}

			#[inline]
			fn class_name() -> ::std::borrow::Cow<'static, #jni::strings::JNIStr> {
				// Safety: we have compile-time encoded the name of the Java class as MUTF8
				// and therefore know it's safe to cast as a JNIStr
				unsafe {
					::std::borrow::Cow::Borrowed(#jni::strings::JNIStr::from_cstr_unchecked(#java_class_cstr))
				}
			}

			#[inline]
			fn lookup_class<'caller>(
				env: &#jni::Env<'_>,
				loader_context: &#jni::refs::LoaderContext,
			) -> #jni::errors::Result<
				impl ::std::ops::Deref<Target = #jni::refs::Global<#jni::objects::JClass<'static>>> + 'caller
			> {
				let api = #api_name::get(env, loader_context)?;
				Ok(&api.class)
			}

			#[inline]
			unsafe fn kind_from_raw<'env>(local_ref: #jni::sys::jobject) -> Self::Kind<'env> {
				unsafe { #type_name(#jni::objects::JObject::kind_from_raw(local_ref)) }
			}

			#[inline]
			unsafe fn global_kind_from_raw(global_ref: #jni::sys::jobject) -> Self::GlobalKind {
				unsafe { #type_name(#jni::objects::JObject::global_kind_from_raw(global_ref)) }
			}
		}
	}
}

/// Generate base methods (`from_raw`, null, `into_raw`)
pub(super) fn generate_base_methods(type_name: &Ident, sys_type: Option<&Ident>, jni: &syn::Path) -> TokenStream {
	// Use the provided sys_type or default to jobject
	let sys_type_ident = sys_type.map_or_else(|| quote! { jobject }, |ident| quote! { #ident });

	// Generate the full path to the sys type
	let sys_type_path = quote! { #jni::sys::#sys_type_ident };

	let jni_path_str = path_to_string_no_spaces(jni);

	quote! {
		impl<'local> #type_name<'local> {
			#[doc = concat!(
				r#"Creates a [`"#, stringify!(#type_name), r#"`] that wraps the given `raw` [jobject]

# Safety

- `raw` must be a valid raw JNI local reference (or `null`).
- `raw` must be an instance of the correct Java class.
- There must not be any other owning [Reference] wrapper for the same reference.
- The local reference must belong to the current thread and not outlive the
  JNI stack frame associated with the [Env] `'local` lifetime.

[jobject]: "#, #jni_path_str, r#"::sys::jobject
[Reference]: "#, #jni_path_str, r#"::refs::Reference
[Env]: "#, #jni_path_str, r#"::Env
"#
			)]
			#[inline]
			pub unsafe fn from_raw<'env_inner>(
				env: &#jni::Env<'env_inner>,
				raw: #sys_type_path,
			) -> #type_name<'env_inner> {
				unsafe { #type_name(#jni::objects::JObject::from_raw(env, raw as #jni::sys::jobject)) }
			}

			#[doc = concat!(
				r#"Creates a new null reference.

Null references are always valid and do not belong to a local reference frame. Therefore,
the returned [`"#, stringify!(#type_name), r#"`] always has the `'static` lifetime."#
			)]
			#[inline]
			pub const fn null() -> #type_name<'static> {
				#type_name(#jni::objects::JObject::null())
			}

			#[doc = r" Unwrap to the raw jni type."]
			#[inline]
			pub fn into_raw(self) -> #sys_type_path {
				self.0.into_raw() as #sys_type_path
			}

			#[doc = concat!(
				r#"Cast a local reference to a [`"#, stringify!(#type_name), r#"`]

This will do a runtime (`IsInstanceOf`) check that the object is an instance of the correct class.

Also see these other options for casting local or global references to a [`"#, stringify!(#type_name), r#"`]:
- [Env::as_cast]("#, #jni_path_str, r#"::Env::as_cast)
- [Env::new_cast_local_ref]("#, #jni_path_str, r#"::Env::new_cast_local_ref)
- [Env::cast_local]("#, #jni_path_str, r#"::Env::cast_local)
- [Env::new_cast_global_ref]("#, #jni_path_str, r#"::Env::new_cast_global_ref)
- [Env::cast_global]("#, #jni_path_str, r#"::Env::cast_global)

# Errors

Returns [Error::WrongObjectType] if the `IsInstanceOf` check fails.

[Error::WrongObjectType]: "#, #jni_path_str, r#"::errors::Error::WrongObjectType
"#
			)]
			#[inline]
			pub fn cast_local<'any_local>(
				env: &mut #jni::Env<'_>,
				obj: impl #jni::refs::Reference
					+ ::core::convert::Into<#jni::objects::JObject<'any_local>>
					+ ::core::convert::AsRef<#jni::objects::JObject<'any_local>>,
			) -> #jni::errors::Result<#type_name<'any_local>> {
				env.cast_local::<#type_name>(obj)
			}
		}
	}
}

/// Generate `is_instance_of` methods and From trait implementations
pub(super) fn generate_is_instance_of_code(
	type_name: &Ident,
	is_instance_of: &[IsInstanceOfEntry],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let mut is_instance_of_methods = Vec::new();
	let mut trait_impls = Vec::new();

	for entry in is_instance_of {
		let type_alias = &entry.type_alias;

		match type_mappings.map_alias(type_alias) {
			Some(ConcreteType::Primitive { .. }) => {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Primitive type '{type_alias}' cannot be used in is_instance_of"),
				));
			}
			Some(ConcreteType::Object {
				reference_type: rust_type,
				..
			}) => {
				let type_path_str = rust_type.path();

				let type_path: syn::Path = syn::parse_str(type_path_str).map_err(|_| {
					syn::Error::new(Span::call_site(), format!("Invalid Rust type path: {type_path_str}"))
				})?;

				if let Some(stem) = &entry.stem {
					let method_name = format_ident!("as_{}", stem);

					is_instance_of_methods.push(quote! {
                        #[doc = concat!(
                        r#"Casts this `"#, stringify!(#type_name), r#"` to a `"#, #type_path_str, r#"`

This does not require a runtime type check since any `"#, stringify!(#type_name), r#"` is also a `"#, #type_path_str, r#"`"#)]
                        pub fn #method_name(&self) -> #jni::refs::Cast<'local, '_, #type_path<'local>> {
                            unsafe { #jni::refs::Cast::<#type_path>::new_unchecked(self) }
                        }
                    });
				}

				// Only generate From impl if the type is not JObject
				if type_alias != "JObject" && type_alias != "abos_jni::objects::JObject" {
					trait_impls.push(quote! {
						impl<'local> ::core::convert::From<#type_name<'local>> for #type_path<'local> {
							fn from(value: #type_name<'local>) -> #type_path<'local> {
								let raw = value.into_raw();
								unsafe { <#type_path as #jni::refs::Reference>::kind_from_raw(raw) }
							}
						}

						// Assuming #type_name<'local> is a transparent wrapper around JObject
						// (asserted) we can implement `AsRef` by casting `&self` to
						// `&#type_path<'local>`.
						impl<'local> ::core::convert::AsRef<#type_path<'local>> for #type_name<'local> {
							fn as_ref(&self) -> &#type_path<'local> {
								const fn assert_is_instance_of_type_is_ffi_safe<T: #jni::refs::TransparentReference>() {}
								const _: () = assert_is_instance_of_type_is_ffi_safe::<#type_path<'_>>();
								// SAFETY: Both reference types are transparent wrappers around JObject,
								// and is_instance_of declares that this object has the target Java type.
								unsafe { &*(self as *const Self).cast::<#type_path<'local>>() }
							}
						}
					});
				}
			}

			None => {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Type '{type_alias}' used in is_instance_of is not defined in types mapping"),
				));
			}
		}
	}

	Ok(quote! {
		impl<'local> #type_name<'local> {
			#(#is_instance_of_methods)*
		}

		#(#trait_impls)*
	})
}

/// Extract cfg attributes from a list of attributes
pub(super) fn extract_cfg_attrs(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
	attrs
		.iter()
		.filter(|attr| attr.path().is_ident("cfg"))
		.cloned()
		.collect()
}
