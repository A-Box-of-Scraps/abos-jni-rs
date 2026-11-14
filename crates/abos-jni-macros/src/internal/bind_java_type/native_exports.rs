use super::{
	Ident, NativeMethod, NativeMethodExport, Result, TokenStream, TypeMappings, create_jni_fn_name, extract_cfg_attrs,
	format_ident, lit_cstr_mutf8, quote,
};

/// Generate the native methods registration code for `API::get()`
pub(super) fn generate_native_registration_code(
	_api_name: &Ident,
	native_methods: &[NativeMethod],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	if native_methods.is_empty() {
		return Ok(quote! {});
	}

	let mut native_method_descriptors: Vec<TokenStream> = Vec::new();
	let mut gated_registrations: Vec<TokenStream> = Vec::new();

	for method in native_methods {
		let java_name: &String = &method.java_name;
		let rust_name: &Ident = &method.rust_name;

		let jni_signature: String = method.signature.to_jni_signature(type_mappings).map_err(|e| {
			syn::Error::new(
				rust_name.span(),
				format!("Failed to generate JNI signature for native method '{rust_name}': {e}"),
			)
		})?;

		let name_cstr: syn::LitCStr = lit_cstr_mutf8(java_name);
		let sig_cstr: syn::LitCStr = lit_cstr_mutf8(&jni_signature);

		// Use wrappers even for raw_fn to retain type checking and export support.
		let wrapper_name: Ident = format_ident!("{}_native_method", rust_name);
		let fn_ptr: TokenStream = quote! { Self::#wrapper_name as *mut ::std::ffi::c_void };

		let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&method.attrs);

		let descriptor: TokenStream = quote! {
			#jni::NativeMethod::from_raw_parts(
				#jni::strings::JNIStr::from_cstr_unchecked(#name_cstr),
				#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
				#fn_ptr,
			)
		};
		if method.requirement.is_required() {
			native_method_descriptors.push(quote! { #(#cfg_attrs)* #descriptor });
		} else {
			let flag: Ident = super::Requirement::native_flag(rust_name);
			let condition: TokenStream = method.requirement.condition();
			let registration: TokenStream = quote! { env.register_native_methods(class, &[#descriptor]) };
			let outcome: TokenStream = if matches!(method.requirement, super::Requirement::Available) {
				quote! {
					match #registration {
						Ok(()) => true,
						Err(#jni::errors::Error::NoSuchMethod(_)) => false,
						Err(error) => return Err(error),
					}
				}
			} else {
				quote! { #registration?; true }
			};
			gated_registrations.push(quote! {
				#(#cfg_attrs)*
				#[allow(unused_variables, reason = "A native without a Rust call wrapper only needs the registration side effect.")]
				let #flag = if #condition {
					// Register one optional native at a time: a missing declaration must
					// not prevent other natives from being registered before class initialization.
					unsafe { #outcome }
				} else { false };
			});
		}
	}

	Ok(quote! {
		{
			// SAFETY: Name and signature literals are validated and encoded as modified UTF-8 at compile time.
			unsafe {
				let native_methods = &[
					#(#native_method_descriptors),*
				];
				if !native_methods.is_empty() {
					env.register_native_methods(class, native_methods)?;
				}
			}
		}
		#(#gated_registrations)*
	})
}

/// Generate exported native method functions for methods marked with export = true
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_native_exports(
	type_name: &Ident,
	api_name: &Ident,
	java_class_dotted: &str,
	native_methods: &[NativeMethod],
	global_native_methods_export: bool,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let mut exports: Vec<TokenStream> = Vec::new();

	for method in native_methods {
		let should_export: bool = match &method.export {
			NativeMethodExport::Default => global_native_methods_export && method.requirement.is_required(),
			NativeMethodExport::No => false,
			NativeMethodExport::WithAutoMangle | NativeMethodExport::WithName(_) => true,
		};

		if !should_export {
			continue;
		}
		if !method.requirement.is_required() {
			return Err(syn::Error::new(
				method.rust_name.span(),
				"Runtime-gated native methods cannot be exported: JNI symbol lookup bypasses registration requirements",
			));
		}

		let export_name: Option<String> = match &method.export {
			NativeMethodExport::WithName(name) => Some(name.clone()),
			_ => None,
		};

		let export: TokenStream = generate_single_native_export(
			type_name,
			api_name,
			java_class_dotted,
			method,
			export_name,
			type_mappings,
			jni,
		)?;
		exports.push(export);
	}

	Ok(quote! {
		#(#exports)*
	})
}

/// Generate a single exported native method function
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_single_native_export(
	type_name: &Ident,
	api_name: &Ident,
	java_class_dotted: &str,
	method: &NativeMethod,
	export_name: Option<String>,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let rust_name: &Ident = &method.rust_name;
	let wrapper_name: Ident = format_ident!("{}_native_method", rust_name);

	let jni_signature: String = method.signature.to_jni_signature(type_mappings).map_err(|e| {
		syn::Error::new(
			rust_name.span(),
			format!("Failed to generate JNI signature for exported native method '{rust_name}': {e}"),
		)
	})?;

	let mangled_name: String =
		export_name.unwrap_or_else(|| create_jni_fn_name(java_class_dotted, &method.java_name, Some(&jni_signature)));
	let mangled_ident: Ident = format_ident!("{}", mangled_name);

	let lifetime: TokenStream = quote! { 'local };

	let params: Vec<TokenStream> = method.native_parameters(type_name, &lifetime, type_mappings, jni);

	let mut call_args: Vec<TokenStream> = vec![quote! { unowned_env }];
	if method.is_static {
		call_args.push(quote! { class });
	} else {
		call_args.push(quote! { this });
	}
	for param in &method.signature.parameters {
		let param_name: &Ident = &param.name;
		call_args.push(quote! { #param_name });
	}

	let return_type: TokenStream =
		crate::internal::types::native_type(&method.signature.return_type, &lifetime, type_mappings, jni);

	let no_mangle_attr: TokenStream = if cfg!(has_unsafe_attr) {
		quote! { #[unsafe(no_mangle)] }
	} else {
		quote! { #[no_mangle] }
	};

	let cfg_attrs: Vec<syn::Attribute> = extract_cfg_attrs(&method.attrs);

	Ok(quote! {
		#(#cfg_attrs)*
		#[doc(hidden)]
		#no_mangle_attr
		#[allow(non_snake_case)]
		pub unsafe extern "system" fn #mangled_ident<#lifetime>(
			#(#params),*
		) -> #return_type {
			#api_name::#wrapper_name(#(#call_args),*)
		}
	})
}
