use super::{
	Ident, JniCallArgs, Method, PrimitiveType, Result, SigType, Span, TokenStream, TypeMappings, format_ident, quote,
	sig_type_to_rust_arg_type, sig_type_to_rust_return_type,
};

/// Generate JNI call arguments processing data for a method/constructor
/// This consolidates the common logic for processing parameters into jvalue arrays
pub(super) fn generate_jni_call_args(
	parameters: &[crate::internal::signature::Parameter],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> JniCallArgs {
	// Generate unique lifetime for each argument
	let lifetimes: Vec<_> = (0..parameters.len())
		.map(|i| {
			let lt_name = format!("local_{i}");
			let lt = syn::Lifetime::new(&format!("'{lt_name}"), Span::call_site());
			quote! { #lt }
		})
		.collect();

	// Use parameter names from the signature
	let names: Vec<_> = parameters.iter().map(|param| param.name.clone()).collect();

	let types: Vec<_> = parameters
		.iter()
		.zip(lifetimes.iter())
		.map(|(param, lt)| sig_type_to_rust_arg_type(&param.ty, lt, type_mappings, jni))
		.collect();

	// Generate jvalue conversion for each argument
	let jvalue_conversions: Vec<_> = parameters
		.iter()
		.zip(names.iter())
		.map(|(param, name)| {
			if let Some(primitive) = param.ty.try_as_primitive(type_mappings) {
				let converter = match primitive {
					PrimitiveType::Boolean => quote! { #jni::objects::JValue::Bool },
					PrimitiveType::Byte => quote! { #jni::objects::JValue::Byte },
					PrimitiveType::Char => quote! { #jni::objects::JValue::Char },
					PrimitiveType::Short => quote! { #jni::objects::JValue::Short },
					PrimitiveType::Int => quote! { #jni::objects::JValue::Int },
					PrimitiveType::Long => quote! { #jni::objects::JValue::Long },
					PrimitiveType::Float => quote! { #jni::objects::JValue::Float },
					PrimitiveType::Double => quote! { #jni::objects::JValue::Double },
					PrimitiveType::Void => quote! { #jni::objects::JValue::Void },
				};
				// Use .into() to cover custom primitive type wrappers
				quote! { #converter((#name).into()).as_jni() }
			} else {
				// For objects and arrays, use as_ref() to get JObject
				quote! { #jni::objects::JValue::Object(#name.as_ref()).as_jni() }
			}
		})
		.collect();

	let decls: Vec<_> = names
		.iter()
		.zip(types.iter())
		.map(|(name, ty)| quote! { #name: #ty })
		.collect();

	JniCallArgs {
		lifetimes,
		jvalue_conversions,
		decls,
	}
}

/// Generate JNI call code for a given return type
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_jni_call_for_return_type(
	debug_name: &str,
	return_type: &SigType,
	is_static: bool,
	jni: &syn::Path,
	this_or_class: &TokenStream,
	method_id: &TokenStream,
	jni_args: &TokenStream,
	type_mappings: &TypeMappings,
) -> TokenStream {
	let call_prefix = if is_static { "CallStatic" } else { "Call" };

	// Determine the JNI function name and return value handling based on the return type
	let (top_check, call_fn, return_tokens) = if let Some(prim) = return_type.try_as_primitive(type_mappings) {
		match prim {
			PrimitiveType::Void => {
				let call_fn = format_ident!("{}VoidMethodA", call_prefix);
				(quote! {}, call_fn, quote! { () })
			}
			PrimitiveType::Boolean => {
				let call_fn = format_ident!("{}BooleanMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Byte => {
				let call_fn = format_ident!("{}ByteMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Char => {
				let call_fn = format_ident!("{}CharMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Short => {
				let call_fn = format_ident!("{}ShortMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Int => {
				let call_fn = format_ident!("{}IntMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Long => {
				let call_fn = format_ident!("{}LongMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Float => {
				let call_fn = format_ident!("{}FloatMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
			PrimitiveType::Double => {
				let call_fn = format_ident!("{}DoubleMethodA", call_prefix);
				(quote! {}, call_fn, quote! { ret })
			}
		}
	} else {
		// For objects, arrays, and rust references, we call CallObjectMethodA
		let call_fn = format_ident!("{}ObjectMethodA", call_prefix);
		let return_type_tokens = sig_type_to_rust_return_type(return_type, &quote! { 'env_local }, type_mappings, jni);
		(
			quote! { env.assert_top(); },
			call_fn,
			quote! { unsafe { <#return_type_tokens>::from_raw(env, ret) } },
		)
	};

	// The JNI spec asserts that `Call<Type>MethodA` _must_ not be called with a `null` object reference
	// Note: we assume that the class will never be null when calling static methods
	let null_check = if is_static {
		quote! {}
	} else {
		let error_msg = format!("Attempted to call '{debug_name}' with a null object");
		quote! {
			if #this_or_class.is_null() {
				return Err(#jni::errors::Error::NullPtr(#error_msg));
			}
		}
	};

	// Generate the JNI call
	quote! {
		{
			// None of the call functions are exception safe, so we can't go ahead with the JNI call
			// if there's currently a pending exception.
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			#null_check

			#top_check

			let ret = unsafe {
				use #jni::refs::Reference as _;
				let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
				let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
				((*interface).v1_1.#call_fn)(
					env_ptr,
					#this_or_class.as_raw(),
					#method_id,
					#jni_args.as_ptr()
				)
			};

			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			Ok(#return_tokens)
		}
	}
}

/// Generate method implementations (instance or static methods)
pub(super) fn generate_methods(
	api_name: &Ident,
	methods: &[Method],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> Result<TokenStream> {
	let mut method_impls = Vec::new();

	for method in methods {
		let rust_name = &method.rust_name;
		let method_id_field = format_ident!("{}_method_id", rust_name);
		let visibility = method.visibility.to_tokens();
		let is_static = method.is_static;

		// Generate JNI call arguments data
		let args = generate_jni_call_args(&method.method_signature.parameters, type_mappings, jni);

		let lifetimes = &args.lifetimes;
		let decls = &args.decls;
		let jvalue_conversions = &args.jvalue_conversions;

		// Determine environment type based on return type (primitives use &Env, objects use &mut Env)
		let env_type = if method
			.method_signature
			.return_type
			.try_as_primitive(type_mappings)
			.is_some()
		{
			quote! { &#jni::Env<'env_local> }
		} else {
			quote! { &mut #jni::Env<'env_local> }
		};

		// Generate return type
		let return_type = sig_type_to_rust_return_type(
			&method.method_signature.return_type,
			&quote! { 'env_local },
			type_mappings,
			jni,
		);

		// Add lifetime declarations for the function
		let lifetime_decls = if lifetimes.is_empty() {
			quote! { <'env_local> }
		} else {
			quote! { <'env_local, #(#lifetimes),*> }
		};

		let self_param = if is_static {
			quote! {}
		} else {
			quote! { &self, }
		};

		// Generate explicit type annotation for jni_args array to handle empty case
		let jni_args_type = if jvalue_conversions.is_empty() {
			quote! { : [#jni::sys::jvalue; 0] }
		} else {
			quote! {}
		};

		// Generate the this_or_class expression
		let this_or_class = if is_static {
			quote! { class }
		} else {
			quote! { self }
		};

		// Generate the method body with JNI call
		let jni_call = generate_jni_call_for_return_type(
			&method.java_name,
			&method.method_signature.return_type,
			is_static,
			jni,
			&this_or_class,
			&quote! { api.#method_id_field.into_raw() },
			&quote! { jni_args },
			type_mappings,
		);

		let class_def = if is_static {
			quote! {
				use #jni::refs::Reference as _;
				let class: &#jni::objects::JClass = api.class.as_ref();
			}
		} else {
			quote! {}
		};

		// Apply attributes to the method
		let attrs = &method.attrs;

		// Generate null check if non_null is true
		let null_check_and_return = if method.non_null {
			quote! {
				#jni_call.and_then(|__result| {
					use #jni::refs::Reference as _;
					if __result.is_null() {
						Err(#jni::errors::Error::NullPtr("Null Object"))
					} else {
						Ok(__result)
					}
				})
			}
		} else {
			quote! {
				#jni_call
			}
		};

		method_impls.push(quote! {
			#(#attrs)*
			#visibility fn #rust_name #lifetime_decls (
				#self_param
				env: #env_type,
				#(#decls),*
			) -> #jni::errors::Result<#return_type> {
				let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
				let jni_args #jni_args_type = [#(#jvalue_conversions),*];

				#class_def
				#null_check_and_return
			}
		});
	}

	// Return just the method implementations without the impl block wrapper
	// The impl block will be created by the caller
	Ok(quote! {
		#(#method_impls)*
	})
}
