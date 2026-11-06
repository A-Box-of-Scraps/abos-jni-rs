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
	signature: MethodSignature,
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
	let abi_check_ensure_env: TokenStream = if is_raw {
		quote! {
			let mut _guard = unsafe {
				#jni::AttachGuard::from_unowned(unowned_env.as_raw())
			};
			let env: &mut #jni::Env<'local> = _guard.borrow_env_mut();
		}
	} else {
		quote! {}
	};
	let abi_assertion: TokenStream = if is_static {
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

	let type_mapping_runtime_checks: TokenStream = check_type_mappings.map_or_else(
		|| quote! {},
		|type_mappings| generate_type_mapping_checks(type_mappings, jni),
	);

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

/// Generate the `NativeMethod` descriptor and optional exported wrapper.
pub fn native_method_impl(input: TokenStream) -> Result<TokenStream> {
	let input: NativeMethodStructInput = syn::parse2(input)?;
	let signature: String = input
		.signature
		.to_jni_signature(&input.type_mappings)
		.map_err(|error| {
			syn::Error::new(
				proc_macro2::Span::call_site(),
				format!("Failed to generate JNI signature: {error}"),
			)
		})?;
	let wrapper = NativeWrapper::new(&input);
	let method: TokenStream =
		wrapper.method_block(&lit_cstr_mutf8(&input.java_method_name), &lit_cstr_mutf8(&signature));
	if matches!(input.export, NativeMethodExport::No) {
		Ok(quote! { const { #method } })
	} else {
		wrapper.export(&signature, &method)
	}
}

struct NativeWrapper<'a> {
	input: &'a NativeMethodStructInput,
	name: syn::Ident,
	lifetime: TokenStream,
	parameters: Vec<TokenStream>,
	arguments: Vec<TokenStream>,
	return_type: TokenStream,
	abi_check: TokenStream,
}

impl<'a> NativeWrapper<'a> {
	fn new(input: &'a NativeMethodStructInput) -> Self {
		let lifetime: TokenStream = quote! { 'local };
		let parameters: Vec<TokenStream> = wrapper_parameters(input, &lifetime);
		let mut arguments: Vec<TokenStream> = vec![if input.is_static {
			quote! { class }
		} else {
			quote! { this }
		}];
		for parameter in &input.signature.parameters {
			let name: &Ident = &parameter.name;
			arguments.push(quote! { #name });
		}
		let return_type: TokenStream = sig_type_to_rust_type_core(
			&input.signature.return_type,
			&lifetime,
			&input.type_mappings,
			&input.jni_crate,
		);
		let abi_check: TokenStream = generate_native_method_abi_check(
			&input.jni_crate,
			&input.java_method_name,
			input.abi_check,
			input.is_raw_fn,
			input.is_static,
			Some(&input.type_mappings),
		);
		Self {
			input,
			name: syn::Ident::new("__native_method_wrapper", proc_macro2::Span::call_site()),
			lifetime,
			parameters,
			arguments,
			return_type,
			abi_check,
		}
	}

	fn call_body(&self) -> TokenStream {
		let input: &NativeMethodStructInput = self.input;
		let jni: &syn::Path = &input.jni_crate;
		let function: &syn::Path = &input.fn_path;
		let arguments: &Vec<TokenStream> = &self.arguments;
		let abi_check: &TokenStream = &self.abi_check;
		if input.is_raw_fn {
			return quote! { #abi_check #function(unowned_env, #(#arguments),*) };
		}
		let with_env_api: TokenStream = if input.catch_unwind {
			quote! { with_env }
		} else {
			quote! { with_env_no_catch }
		};
		let policy: TokenStream = input.error_policy.as_ref().map_or_else(
			|| quote! { #jni::errors::ThrowRuntimeExAndDefault },
			|path| quote! { #path },
		);
		quote! {
			unowned_env.#with_env_api(|env| { #abi_check #function(env, #(#arguments),*) }).resolve::<#policy>()
		}
	}

	fn method_block(&self, name_cstr: &syn::LitCStr, sig_cstr: &syn::LitCStr) -> TokenStream {
		let jni: &syn::Path = &self.input.jni_crate;
		let name: &Ident = &self.name;
		let lifetime: &TokenStream = &self.lifetime;
		let parameters: &Vec<TokenStream> = &self.parameters;
		let return_type: &TokenStream = &self.return_type;
		let body: TokenStream = self.call_body();
		quote! {
			extern "system" fn #name<#lifetime>(#(#parameters),*) -> #return_type { #body }
			// The generated wrapper enforces the declared signature and ABI; literals are validated modified UTF-8.
			unsafe {
				#jni::NativeMethod::from_raw_parts(
					#jni::strings::JNIStr::from_cstr_unchecked(#name_cstr),
					#jni::strings::JNIStr::from_cstr_unchecked(#sig_cstr),
					#name as *mut ::std::ffi::c_void,
				)
			}
		}
	}

	fn export(&self, signature: &str, method: &TokenStream) -> Result<TokenStream> {
		let input: &NativeMethodStructInput = self.input;
		let mangled_name: String = if let NativeMethodExport::WithName(name) = &input.export {
			name.clone()
		} else {
			let class: &JavaClassName = input.java_type.as_ref().ok_or_else(|| {
				syn::Error::new_spanned(
					&input.fn_path,
					"java_type is required for an automatically mangled export",
				)
			})?;
			create_jni_fn_name(&class.to_java_dotted(), &input.java_method_name, Some(signature))
		};
		let export_attr: TokenStream = if cfg!(has_unsafe_attr) {
			quote! { #[unsafe(export_name = #mangled_name)] }
		} else {
			quote! { #[export_name = #mangled_name] }
		};
		let name: &Ident = &self.name;
		let lifetime: &TokenStream = &self.lifetime;
		let parameters: &Vec<TokenStream> = &self.parameters;
		let arguments: &Vec<TokenStream> = &self.arguments;
		let return_type: &TokenStream = &self.return_type;
		Ok(quote! {
			const {
				#[doc(hidden)]
				#export_attr
				pub extern "system" fn __native_method_export<#lifetime>(#(#parameters),*) -> #return_type {
					#name(unowned_env, #(#arguments),*)
				}
				#method
			}
		})
	}
}

fn wrapper_parameters(input: &NativeMethodStructInput, lifetime: &TokenStream) -> Vec<TokenStream> {
	let jni: &syn::Path = &input.jni_crate;
	let mut parameters: Vec<TokenStream> = vec![if input.is_raw_fn {
		quote! { unowned_env: #jni::EnvUnowned<#lifetime> }
	} else {
		quote! { mut unowned_env: #jni::EnvUnowned<#lifetime> }
	}];
	if input.is_static {
		parameters.push(quote! { class: #jni::objects::JClass<#lifetime> });
	} else {
		let ty: TokenStream = input.rust_type.as_ref().map_or_else(
			|| quote! { #jni::objects::JObject<#lifetime> },
			|path| quote! { #path<#lifetime> },
		);
		parameters.push(quote! { this: #ty });
	}
	for parameter in &input.signature.parameters {
		let name: &Ident = &parameter.name;
		let ty: TokenStream = sig_type_to_rust_type_core(&parameter.ty, lifetime, &input.type_mappings, jni);
		parameters.push(quote! { #name: #ty });
	}
	parameters
}

mod parse;

#[cfg(test)]
mod tests;
