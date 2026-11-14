use super::{
	Ident, JniCallArgs, Method, PrimitiveType, SigType, Span, TokenStream, TypeMappings, format_ident, quote,
	sig_type_to_rust_arg_type, sig_type_to_rust_return_type,
};

/// Generate `jvalue` array entries for method or constructor arguments.
pub(super) fn generate_jni_call_args(
	parameters: &[crate::internal::signature::Parameter],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> JniCallArgs {
	let lifetimes: Vec<TokenStream> = (0..parameters.len())
		.map(|i| {
			let lt_name: String = format!("local_{i}");
			let lt = syn::Lifetime::new(&format!("'{lt_name}"), Span::call_site());
			quote! { #lt }
		})
		.collect();

	let names: Vec<Ident> = parameters.iter().map(|param| param.name.clone()).collect();

	let types: Vec<TokenStream> = parameters
		.iter()
		.zip(lifetimes.iter())
		.map(|(param, lt)| sig_type_to_rust_arg_type(&param.ty, lt, type_mappings, jni))
		.collect();

	let jvalue_conversions: Vec<TokenStream> = parameters
		.iter()
		.zip(names.iter())
		.map(|(param, name)| {
			param.ty.try_as_primitive(type_mappings).map_or_else(
				|| quote! { #jni::objects::JValue::Object(#name.as_ref()).as_jni() },
				|primitive| {
					let converter: TokenStream = match primitive {
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
				},
			)
		})
		.collect();

	let decls: Vec<TokenStream> = names
		.iter()
		.zip(types.iter())
		.map(|(name, ty)| quote! { #name: #ty })
		.collect();

	let reference_args: Vec<TokenStream> = parameters
		.iter()
		.enumerate()
		.filter_map(|(index, param)| {
			if param.ty.try_as_primitive(type_mappings).is_some() {
				None
			} else {
				let name: &Ident = &param.name;
				Some(quote! { (#index, #name) })
			}
		})
		.collect();

	let reference_bindings: Vec<TokenStream> = parameters
		.iter()
		.filter(|param| param.ty.try_as_primitive(type_mappings).is_none())
		.map(|param| {
			let name: &Ident = &param.name;
			quote! { let #name: &#jni::objects::JObject = #name.as_ref().as_ref(); }
		})
		.collect();

	JniCallArgs {
		lifetimes,
		jvalue_conversions,
		reference_args,
		reference_bindings,
		decls,
	}
}

/// Generate JNI call code for a given return type
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
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
	let call_prefix: &str = if is_static { "CallStatic" } else { "Call" };

	let (top_check, call_fn, return_tokens) = return_type.try_as_primitive(type_mappings).map_or_else(
		|| {
			let call_fn: Ident = format_ident!("{}ObjectMethodA", call_prefix);
			let return_type_tokens: TokenStream =
				sig_type_to_rust_return_type(return_type, &quote! { 'env_local }, type_mappings, jni);
			(
				quote! { env.assert_top(); },
				call_fn,
				quote! {
					{
						// The declared Java type may resolve through a different class loader
						// than the mapped Rust wrapper's cached class.
						let object = unsafe { #jni::objects::JObject::from_raw(env, ret) };
						env.cast_local::<#return_type_tokens>(object)?
					}
				},
			)
		},
		|prim| {
			let call_fn: Ident = format_ident!("{}{}MethodA", call_prefix, prim.jni_name());
			let value: TokenStream = primitive_return(&prim, return_type, type_mappings, jni);
			(quote! {}, call_fn, value)
		},
	);

	// JNI instance calls require a non-null receiver; cached static classes are non-null.
	let null_check: TokenStream = generate_receiver_null_check(
		is_static,
		this_or_class,
		&format!("Attempted to call '{debug_name}' with a null object"),
		jni,
	);

	generate_checked_jni_call(
		jni,
		&call_fn,
		&quote! { #this_or_class.as_raw(), #method_id, #jni_args.as_ptr() },
		&quote! { #null_check #top_check },
		&return_tokens,
	)
}

/// Convert raw JNI method and field results into the declared Rust primitive type.
pub(super) fn primitive_return(
	primitive: &PrimitiveType,
	ty: &SigType,
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	if *primitive == PrimitiveType::Void {
		quote! { () }
	} else if crate::internal::types::is_rust_bool(ty, type_mappings) {
		quote! { ret != #jni::sys::JNI_FALSE }
	} else {
		quote! { ret.into() }
	}
}

/// Generate method implementations (instance or static methods)
pub(super) fn generate_methods(
	api_name: &Ident,
	methods: &[Method],
	type_mappings: &TypeMappings,
	jni: &syn::Path,
) -> TokenStream {
	let mut method_impls: Vec<TokenStream> = Vec::new();

	for method in methods {
		method_impls.push(generate_method(api_name, method, type_mappings, jni));
	}

	// The caller supplies the enclosing impl block.
	quote! {
		#(#method_impls)*
	}
}

fn generate_method(api_name: &Ident, method: &Method, type_mappings: &TypeMappings, jni: &syn::Path) -> TokenStream {
	let rust_name: &Ident = &method.rust_name;
	let method_id_field: Ident = format_ident!("{}_method_id", rust_name);
	let local_id: Ident = super::Requirement::local_id(&method_id_field);
	let id_access: TokenStream = method.requirement.access_id(&method_id_field, &method.java_name, jni);
	let visibility: TokenStream = method.visibility.to_tokens();
	let is_static: bool = method.is_static;

	let args: JniCallArgs = generate_jni_call_args(&method.signature.parameters, type_mappings, jni);

	let validation: TokenStream = args.reference_validation(&local_id, is_static, jni);
	let decls: &Vec<TokenStream> = &args.decls;
	let jvalue_conversions: &Vec<TokenStream> = &args.jvalue_conversions;

	let env_type: TokenStream = if method.signature.return_type.try_as_primitive(type_mappings).is_some() {
		quote! { &#jni::Env<'env_local> }
	} else {
		quote! { &mut #jni::Env<'env_local> }
	};

	let return_type: TokenStream = sig_type_to_rust_return_type(
		&method.signature.return_type,
		&quote! { 'env_local },
		type_mappings,
		jni,
	);

	let lifetime_decls: TokenStream = args.lifetime_declarations();

	let self_param: TokenStream = if is_static {
		quote! {}
	} else {
		quote! { &self, }
	};

	// Empty argument arrays need an explicit element type.
	let jni_args_type: TokenStream = args.array_annotation(jni);

	let this_or_class: TokenStream = if is_static {
		quote! { class }
	} else {
		quote! { self }
	};

	let jni_call: TokenStream = generate_jni_call_for_return_type(
		&method.java_name,
		&method.signature.return_type,
		is_static,
		jni,
		&this_or_class,
		&quote! { #local_id.into_raw() },
		&quote! { jni_args },
		type_mappings,
	);

	let class_def: TokenStream = if is_static {
		quote! {
			use #jni::refs::Reference as _;
			let class: &#jni::objects::JClass = api.class.as_ref();
		}
	} else {
		quote! {}
	};

	let attrs: &Vec<syn::Attribute> = &method.attrs;

	let null_check_and_return: TokenStream = checked_return(&jni_call, method.non_null, jni);

	quote! {
		#(#attrs)*
		#visibility fn #rust_name #lifetime_decls (
			#self_param
			env: #env_type,
			#(#decls),*
		) -> #jni::errors::Result<#return_type> {
			let api = #api_name::get(env, &#jni::refs::LoaderContext::None)?;
			#id_access
			#validation
			let jni_args #jni_args_type = [#(#jvalue_conversions),*];

			#class_def
			#null_check_and_return
		}
	}
}

pub(super) fn checked_return(jni_call: &TokenStream, non_null: bool, jni: &syn::Path) -> TokenStream {
	if non_null {
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
	}
}

impl JniCallArgs {
	pub(super) fn reference_validation(&self, method_id: &Ident, is_static: bool, jni: &syn::Path) -> TokenStream {
		let references: &Vec<TokenStream> = &self.reference_args;
		let bindings: &Vec<TokenStream> = &self.reference_bindings;
		if references.is_empty() {
			return quote! {};
		}
		quote! {
			// Evaluate AsRef once so validation and invocation use the same objects.
			#(#bindings)*
			static __JNI_REFERENCE_TYPES: #jni::__MethodReferenceCache = #jni::__MethodReferenceCache::new();
			// SAFETY: This binding always uses the same ID and reference indices. Its
			// process-wide API retains the resolved class for the cache's lifetime.
			unsafe {
				__JNI_REFERENCE_TYPES.validate(
					env, api.class.as_ref(), #method_id.into_raw(), #is_static, &[#(#references),*],
				)?;
			}
		}
	}

	fn lifetime_declarations(&self) -> TokenStream {
		let lifetimes: &Vec<TokenStream> = &self.lifetimes;
		if lifetimes.is_empty() {
			quote! { <'env_local> }
		} else {
			quote! { <'env_local, #(#lifetimes),*> }
		}
	}

	fn array_annotation(&self, jni: &syn::Path) -> TokenStream {
		if self.jvalue_conversions.is_empty() {
			quote! { : [#jni::sys::jvalue; 0] }
		} else {
			quote! {}
		}
	}
}

/// Keep exception checks around the raw call, including before allocating a local reference.
pub(super) fn generate_checked_jni_call(
	jni: &syn::Path,
	call_fn: &Ident,
	args: &TokenStream,
	preflight: &TokenStream,
	return_tokens: &TokenStream,
) -> TokenStream {
	quote! {
		{
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}
			#preflight
			let ret = unsafe {
				use #jni::refs::Reference as _;
				let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
				let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
				((*interface).v1_1.#call_fn)(env_ptr, #args)
			};
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}
			Ok(#return_tokens)
		}
	}
}

pub(super) fn generate_receiver_null_check(
	is_static: bool,
	receiver: &TokenStream,
	message: &str,
	jni: &syn::Path,
) -> TokenStream {
	if is_static {
		quote! {}
	} else {
		quote! {
			if #receiver.is_null() {
				return Err(#jni::errors::Error::NullPtr(#message));
			}
		}
	}
}
