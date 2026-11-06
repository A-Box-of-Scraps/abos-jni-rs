use super::{
	Ident, NativeMethod, NativeMethodExport, Result, TokenStream, TypeMappings, create_jni_fn_name, extract_cfg_attrs,
	format_ident, lit_cstr_mutf8, quote, sig_type_to_rust_native_trait_arg_type, sig_type_to_rust_return_type,
};

/// Generate the native methods registration code for `API::get()`
pub(super) fn generate_native_registration_code(
	_api_name: &Ident,
	native_methods: &[NativeMethod],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	// If no native methods, return empty
	if native_methods.is_empty() {
		return Ok(quote! {});
	}

	let mut native_method_descriptors = Vec::new();

	for method in native_methods {
		let java_name = &method.java_name;
		let rust_name = &method.rust_name;

		// Use MethodSignature to generate JNI signature
		let jni_signature = method.signature.to_jni_signature(type_mappings).map_err(|e| {
			syn::Error::new(
				rust_name.span(),
				format!("Failed to generate JNI signature for native method '{rust_name}': {e}"),
			)
		})?;

		// Create CStr literals for name and signature
		let name_cstr = lit_cstr_mutf8(java_name);
		let sig_cstr = lit_cstr_mutf8(&jni_signature);

		// Always use the generated wrapper function (even for raw_fn)
		// This ensures type checking and allows exports to work
		let wrapper_name = format_ident!("{}_native_method", rust_name);
		let fn_ptr = quote! { Self::#wrapper_name as *mut ::std::ffi::c_void };

		// Extract cfg attributes to guard the registration descriptor
		let cfg_attrs = extract_cfg_attrs(&method.attrs);

		native_method_descriptors.push(quote! {
			#(#cfg_attrs)*
			#jni::NativeMethod::from_raw_parts(
				#jni::strings::JNIStr::from_cstr_unchecked(#name_cstr),
				#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
				#fn_ptr,
			)
		});
	}

	Ok(quote! {
		{
			// Safety: All of the name and signature CStr literals have been validate
			// at compile time and encoded as MUTF-8 - therefore they can be safely
			// cast to a JNIStr unchecked at runtime.
			unsafe {
				let native_methods = &[
					#(#native_method_descriptors),*
				];
				env.register_native_methods(class, native_methods)?;
			}
		}
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
	let mut exports = Vec::new();

	for method in native_methods {
		// Determine if this method should be exported
		let should_export = match &method.export {
			NativeMethodExport::Default => global_native_methods_export,
			NativeMethodExport::No => false,
			NativeMethodExport::WithAutoMangle | NativeMethodExport::WithName(_) => true,
		};

		if !should_export {
			continue;
		}

		// Determine the export name (None = auto-mangle)
		let export_name = match &method.export {
			NativeMethodExport::WithName(name) => Some(name.clone()),
			_ => None,
		};

		let export = generate_single_native_export(
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
	let rust_name = &method.rust_name;
	let wrapper_name = format_ident!("{}_native_method", rust_name);

	// Use MethodSignature to generate JNI signature for mangling
	let jni_signature = method.signature.to_jni_signature(type_mappings).map_err(|e| {
		syn::Error::new(
			rust_name.span(),
			format!("Failed to generate JNI signature for exported native method '{rust_name}': {e}"),
		)
	})?;

	// Generate the mangled JNI function name
	let mangled_name =
		export_name.unwrap_or_else(|| create_jni_fn_name(java_class_dotted, &method.java_name, Some(&jni_signature)));
	let mangled_ident = format_ident!("{}", mangled_name);

	let lifetime = quote! { 'local };

	// Build parameter list for export function (same as wrapper)
	let mut params = Vec::new();
	params.push(quote! { mut unowned_env: #jni::EnvUnowned<#lifetime> });

	if method.is_static {
		params.push(quote! { class: #jni::objects::JClass<#lifetime> });
	} else {
		params.push(quote! { this: #type_name<#lifetime> });
	}

	// Add method parameters
	for param in &method.signature.parameters {
		let param_name = &param.name;
		let rust_type = sig_type_to_rust_native_trait_arg_type(&param.ty, &lifetime, type_mappings, jni);
		params.push(quote! { #param_name: #rust_type });
	}

	// Build forwarding call arguments
	let mut call_args = vec![quote! { unowned_env }];
	if method.is_static {
		call_args.push(quote! { class });
	} else {
		call_args.push(quote! { this });
	}
	for param in &method.signature.parameters {
		let param_name = &param.name;
		call_args.push(quote! { #param_name });
	}

	// Get return type
	let return_type = sig_type_to_rust_return_type(&method.signature.return_type, &lifetime, type_mappings, jni);

	let no_mangle_attr = if cfg!(has_unsafe_attr) {
		quote! { #[unsafe(no_mangle)] }
	} else {
		quote! { #[no_mangle] }
	};

	let cfg_attrs = extract_cfg_attrs(&method.attrs);

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
