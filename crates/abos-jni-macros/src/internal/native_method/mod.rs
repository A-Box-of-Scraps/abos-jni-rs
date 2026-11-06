//! Procedural macros for generating compile-time type-checked `NativeMethod`
//! structs
//!
//! This module implements the `native_method!` macro that create `NativeMethod`
//! descriptors with compile-time guarantees that the function pointer matches
//! the JNI signature.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	Ident, Result, Token, custom_keyword,
	ext::IdentExt,
	parse::{Parse, ParseStream},
};

use crate::internal::types::{TypeMappings, parse_type};
use crate::internal::types::{generate_type_mapping_checks, sig_type_to_rust_type_core};
use crate::internal::{
	mangle::{create_jni_fn_name, snake_case_to_lower_camel_case},
	types::SigType,
};
use crate::internal::{
	signature::{MethodSignature, parse_method_sig, parse_parameter_with_index},
	types::AbiCheck,
};
use crate::internal::{str::lit_cstr_mutf8, types::JavaClassName};

custom_keyword!(jni);
custom_keyword!(rust_type);
custom_keyword!(java_type);
custom_keyword!(name);
custom_keyword!(sig);
custom_keyword!(type_map);
custom_keyword!(error);
custom_keyword!(error_policy);
custom_keyword!(export);
custom_keyword!(raw);
custom_keyword!(abi_check);
custom_keyword!(catch_unwind);

/// Represents the export behavior for a native method
#[derive(Clone)]
pub enum NativeMethodExport {
	/// Use the default behavior
	Default,
	/// Don't export this method (override global setting)
	No,
	/// Export with automatically mangled JNI name
	WithAutoMangle,
	/// Export with explicitly provided name
	WithName(String),
}

/// Input structure for `native_method`!
struct NativeMethodStructInput {
	is_static: bool,
	jni_crate: syn::Path,
	rust_type: Option<syn::Path>,
	/// Java class name for export (e.g., "com.example.MyClass")
	java_type: Option<JavaClassName>,
	java_method_name: String,
	method_signature: MethodSignature,
	fn_path: syn::Path,
	type_mappings: TypeMappings,
	/// true if `raw = true` or `raw` qualifier with inline signature
	is_raw_fn: bool,
	/// Only used when `is_raw_fn` is false
	error_policy: Option<Ident>,
	/// `WithAutoMangle` if `export = true` or `extern` qualifier with inline signature
	export: NativeMethodExport,
	/// ABI check policy, for checking the type of the 'this'/'class' parameter at runtime
	abi_check: AbiCheck,
	/// non-raw wrapper uses `EnvUnowned::with_env` if true, else `with_env_no_catch`
	catch_unwind: bool,
}

pub fn generate_native_method_abi_check(
	jni: &syn::Path,
	method_name: &str,
	need_abi_check: AbiCheck,
	is_raw: bool,
	is_static: bool,
	check_type_mappings: Option<&TypeMappings>,
) -> TokenStream {
	if !need_abi_check.requires_abi_check() {
		return quote! {};
	}

	// Check at runtime that the method was correctly registered as static or instance
	// We assume this can't change dynamically so we use Once to only check once
	let abi_check_ensure_env = if is_raw {
		quote! {
			let mut _guard = unsafe {
				#jni::AttachGuard::from_unowned(unowned_env.as_raw())
			};
			let env: &mut #jni::Env<'local> = _guard.borrow_env_mut();
		}
	} else {
		quote! {}
	};
	let abi_assertion = if is_static {
		quote! {
			let is_class_reciever = env.is_instance_of(&class, lang_class_class).expect("Failed to check 2nd arg type for native method ABI check");
			assert!(is_class_reciever, "Native method '{}' was registered as static but called as instance method", stringify!(#method_name));
		}
	} else {
		quote! {
			let is_class_reciever = env.is_instance_of(&this, lang_class_class).expect("Failed to check 2nd arg type for native method ABI check");
			assert!(!is_class_reciever, "Native method '{}' was registered as instance but called as static method", stringify!(#method_name));
		}
	};

	let type_mapping_runtime_checks = if let Some(type_mappings) = check_type_mappings {
		generate_type_mapping_checks(type_mappings, jni)
	} else {
		quote! {}
	};

	quote! {
		// Use atomic try_update to guard the ABI check. The closure runs without holding
		// any lock, so multiple threads may race and perform the check concurrently on first
		// invocation. This is acceptable since the check should always succeed or fail consistently.
		//
		// Critically: we only set the flag to true if the checks succeed. If a check fails and
		// panics, the flag remains false, forcing subsequent threads to retry the check (in case
		// the program doesn't abort).
		//
		// This approach avoids locks that could cause deadlocks if the check triggers class
		// initialization which calls back into native methods.
		static _ABI_CHECK_DONE: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false);

		// The closure returns Some(true) only after the check succeeds.
		// If the check panics, the flag stays false and other attempts to call this method will
		// retry the check (in case the program doesn't abort).
		let _ = _ABI_CHECK_DONE.try_update(
			::std::sync::atomic::Ordering::AcqRel,   // set ordering
			::std::sync::atomic::Ordering::Acquire,   // fetch ordering
			|done| {
				if done {
					// The checks have already been completed successfully
					None
				} else {
					// Perform the ABI check (runs outside any lock)
					#abi_check_ensure_env

					// Note: The `jni` crate has a MSRV that lets us assume it's safe to panic and unwind
					// up to a JNI boundary and then abort. Therefore we can use expect + assertions here.
					let lang_class_class = <#jni::objects::JClass as #jni::refs::Reference>::lookup_class(
						env,
						&#jni::refs::LoaderContext::None,
					).expect("Failed to lookup Java class for java.lang.class, for native method ABI check");
					let lang_class_class: &#jni::objects::JClass = &lang_class_class;
					#abi_assertion
					#type_mapping_runtime_checks

					// Check succeeded - set flag to true
					Some(true)
				}
			}
		);
		// If try_update returned Ok, we successfully set the flag (check passed)
		// If it returned Err, another thread beat us to it (also check passed)
		// If the check failed, we panicked and never reached here
	}
}

/// Generate the `NativeMethod` struct creation
pub fn native_method_impl(input: TokenStream) -> Result<TokenStream> {
	let input: NativeMethodStructInput = syn::parse2(input)?;

	let is_static = input.is_static;
	let jni = &input.jni_crate;
	let java_method_name = &input.java_method_name;
	let fn_path = &input.fn_path;
	let type_mappings = &input.type_mappings;

	// Generate JNI signature
	let jni_signature = input.method_signature.to_jni_signature(type_mappings).map_err(|e| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			format!("Failed to generate JNI signature: {e}"),
		)
	})?;

	// Create CStr literals for name and signature
	let name_cstr = lit_cstr_mutf8(java_method_name);
	let sig_cstr = lit_cstr_mutf8(&jni_signature);

	// Generate the type-checked wrapper function
	let lifetime = quote! { 'local };

	// Build parameter list - differs for raw vs wrapped functions
	// For raw functions: extern "system" signature with EnvUnowned<'local> (no &mut)
	// For wrapped functions: the user's function takes &mut Env<'local>
	let mut raw_params = Vec::new();
	let mut user_fn_params = Vec::new();

	if input.is_raw_fn {
		// Raw function: extern "system" fn(unowned_env: EnvUnowned<'local>, ...)
		raw_params.push(quote! { unowned_env: #jni::EnvUnowned<#lifetime> });
		user_fn_params.push(quote! { unowned_env: #jni::EnvUnowned<#lifetime> });
	} else {
		// Wrapped function: extern "system" wrapper, but user's fn takes &mut Env<'local>
		raw_params.push(quote! { mut unowned_env: #jni::EnvUnowned<#lifetime> });
		user_fn_params.push(quote! { env: &mut #jni::Env<#lifetime> });
	}

	// Add this/class parameter
	if is_static {
		raw_params.push(quote! { class: #jni::objects::JClass<#lifetime> });
		user_fn_params.push(quote! { class: #jni::objects::JClass<#lifetime> });
	} else {
		let this_type = if let Some(rust_type) = &input.rust_type {
			quote! { #rust_type<#lifetime> }
		} else {
			quote! { #jni::objects::JObject<#lifetime> }
		};
		raw_params.push(quote! { this: #this_type });
		user_fn_params.push(quote! { this: #this_type });
	}

	// Add method parameters
	for param in &input.method_signature.parameters {
		let param_name = &param.name;
		let rust_type = sig_type_to_rust_type_core(&param.ty, &lifetime, type_mappings, jni);
		raw_params.push(quote! { #param_name: #rust_type });
		user_fn_params.push(quote! { #param_name: #rust_type });
	}

	// Get return type
	let return_type = sig_type_to_rust_type_core(&input.method_signature.return_type, &lifetime, type_mappings, jni);

	// Build argument list for calling the user's function
	let mut call_args = Vec::new();
	if is_static {
		call_args.push(quote! { class });
	} else {
		call_args.push(quote! { this });
	}
	for param in &input.method_signature.parameters {
		let param_name = &param.name;
		call_args.push(quote! { #param_name });
	}

	// Note: we unconditionally generate a wrapper, even for raw functions without
	// an ABI check, so we can be sure we register an `extern "system"` ABI function
	// pointer.

	let abi_check = generate_native_method_abi_check(
		jni,
		java_method_name,
		input.abi_check,
		input.is_raw_fn,
		input.is_static,
		Some(type_mappings),
	);

	let (wrapper_path, native_method_block) = if input.is_raw_fn {
		// For raw functions with ABI check: create wrapper that does ABI check
		// The wrapper has extern "system" signature with raw_params
		// The user's function is called with user_fn_params (EnvUnowned)
		let wrapper_ident = syn::Ident::new("__native_method_wrapper", proc_macro2::Span::call_site());
		let wrapper_path: syn::Path = syn::parse_quote! { #wrapper_ident };

		let block = quote! {
			// ABI-checking wrapper function that converts from extern "system" to Rust calling convention
			extern "system" fn #wrapper_ident<#lifetime>(
				#(#raw_params),*
			) -> #return_type {
				#abi_check

				// Call the user's raw function
				#fn_path(unowned_env, #(#call_args),*)
			}

			// Safety: The wrapper function is type-checked at compile time to match
			// the signature specified in the macro. The function pointer is valid
			// and points to __native_method_abi_check_wrapper.
			unsafe {
				#jni::NativeMethod::from_raw_parts(
					#jni::strings::JNIStr::from_cstr_unchecked(#name_cstr),
					#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
					#wrapper_ident as *mut ::std::ffi::c_void,
				)
			}
		};
		(wrapper_path, block)
	} else {
		let with_env_api = if input.catch_unwind {
			quote! { with_env }
		} else {
			quote! { with_env_no_catch }
		};
		// For safe wrapped functions: create wrapper that calls with_env and resolve
		// The wrapper has extern "system" signature with raw_params
		// The user's function is called with user_fn_params (env: &mut Env)
		// Type checking happens naturally when calling the user function
		let error_policy = input
			.error_policy
			.as_ref()
			.map_or_else(|| quote! { #jni::errors::ThrowRuntimeExAndDefault }, |p| quote! { #p });

		let wrapper_ident = syn::Ident::new("__native_method_wrapper", proc_macro2::Span::call_site());
		let wrapper_path: syn::Path = syn::parse_quote! { #wrapper_ident };

		let block = quote! {
			// Type-checked wrapper function that converts from extern "system" to Rust calling convention
			// and handles error resolution
			extern "system" fn #wrapper_ident<#lifetime>(
				#(#raw_params),*
			) -> #return_type {
				unowned_env
					.#with_env_api(|env| {
						#abi_check

						#fn_path(env, #(#call_args),*)
					})
					.resolve::<#error_policy>()
			}

			// Safety: The wrapper function is type-checked at compile time to match
			// the signature specified in the macro. The function pointer is valid
			// and points to __native_method_wrapper.
			unsafe {
				#jni::NativeMethod::from_raw_parts(
					#jni::strings::JNIStr::from_cstr_unchecked(#name_cstr),
					#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
					#wrapper_ident as *mut ::std::ffi::c_void,
				)
			}
		};
		(wrapper_path, block)
	};

	// Generate export wrapper if requested
	if matches!(input.export, NativeMethodExport::No) {
		Ok(quote! {
			const {
				#native_method_block
			}
		})
	} else {
		// Generate the mangled JNI function name
		let mangled_name = if let NativeMethodExport::WithName(export_name) = input.export {
			export_name
		} else {
			let java_class = input.java_type.as_ref().ok_or_else(|| {
				syn::Error::new_spanned(
					&input.fn_path,
					"java_type is required for an automatically mangled export",
				)
			})?;
			create_jni_fn_name(&java_class.to_java_dotted(), java_method_name, Some(&jni_signature))
		};

		// Use export_name attribute - check if has_unsafe_attr is set
		let export_name_attr = if cfg!(has_unsafe_attr) {
			quote! { #[unsafe(export_name = #mangled_name)] }
		} else {
			quote! { #[export_name = #mangled_name] }
		};

		// Build call arguments for the wrapper (just pass through all params)
		let mut export_call_args = vec![quote! { unowned_env }];
		if is_static {
			export_call_args.push(quote! { class });
		} else {
			export_call_args.push(quote! { this });
		}
		for param in &input.method_signature.parameters {
			let param_name = &param.name;
			export_call_args.push(quote! { #param_name });
		}

		// The export wrapper calls the wrapper function (for wrapped) or user function (for raw)
		Ok(quote! {
			const {
				#[doc(hidden)]
				#export_name_attr
				pub extern "system" fn __native_method_export<#lifetime>(
					#(#raw_params),*
				) -> #return_type {
					#wrapper_path(#(#export_call_args),*)
				}

				#native_method_block
			}
		})
	}
}

mod parse;

#[cfg(test)]
mod tests;
