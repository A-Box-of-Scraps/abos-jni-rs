use super::{PrimitiveType, SigType, TokenStream, TypeMappings, format_ident, quote, sig_type_to_rust_return_type};

/// Generate JNI get field call code for a given field type
pub(super) fn generate_jni_get_field_call(
	debug_name: &str,
	field_type: &SigType,
	is_static: bool,
	jni: &syn::Path,
	this_or_class: &TokenStream,
	field_id: &TokenStream,
	type_mappings: &TypeMappings,
) -> TokenStream {
	let call_prefix: &str = if is_static { "GetStatic" } else { "Get" };

	let (top_check, call_fn, return_tokens) = if let Some(prim) = field_type.try_as_primitive(type_mappings) {
		if prim == PrimitiveType::Void {
			return quote! { Ok(()) };
		}
		let call_fn: proc_macro2::Ident = format_ident!("{}{}Field", call_prefix, prim.jni_name());
		let value: TokenStream = super::calls::primitive_return(&prim, field_type, type_mappings, jni);
		(quote! {}, call_fn, value)
	} else {
		let call_fn: proc_macro2::Ident = format_ident!("{}ObjectField", call_prefix);
		let return_type_tokens: TokenStream =
			sig_type_to_rust_return_type(field_type, &quote! { 'env_local }, type_mappings, jni);
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
	};

	// JNI instance field access requires a non-null receiver; cached static classes are non-null.
	let null_check: TokenStream = super::calls::generate_receiver_null_check(
		is_static,
		this_or_class,
		&format!("Attempted to set '{debug_name}' field with a null object"),
		jni,
	);

	super::calls::generate_checked_jni_call(
		jni,
		&call_fn,
		&quote! { #this_or_class.as_raw(), #field_id },
		&quote! { #null_check #top_check },
		&return_tokens,
	)
}

/// Generate JNI set field call code for a given field type
#[allow(
	clippy::too_many_arguments,
	reason = "Code generation needs the independent binding metadata and JNI paths."
)]
pub(super) fn generate_jni_set_field_call(
	debug_name: &str,
	field_type: &SigType,
	is_static: bool,
	jni: &syn::Path,
	this_or_class: &TokenStream,
	field_id: &TokenStream,
	val: &TokenStream,
	type_mappings: &TypeMappings,
) -> TokenStream {
	let call_prefix: &str = if is_static { "SetStatic" } else { "Set" };

	let (call_fn, val_conversion) = if let Some(prim) = field_type.try_as_primitive(type_mappings) {
		if prim == PrimitiveType::Void {
			return quote! { Ok(()) };
		}
		let call_fn: proc_macro2::Ident = format_ident!("{}{}Field", call_prefix, prim.jni_name());
		(call_fn, quote! { (#val).into() })
	} else {
		let call_fn: proc_macro2::Ident = format_ident!("{}ObjectField", call_prefix);
		(call_fn, quote! { #val.as_raw() })
	};

	// JNI instance field access requires a non-null receiver; cached static classes are non-null.
	let null_check: TokenStream = super::calls::generate_receiver_null_check(
		is_static,
		this_or_class,
		&format!("Attempted to set '{debug_name}' field with a null object"),
		jni,
	);

	quote! {
		{
			// Run user conversions before the exception check guarding the JNI call.
			let jni_value = #val_conversion;
			// JNI field setters must not run with a pending exception.
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			#null_check

			unsafe {
				use #jni::refs::Reference as _;
				let env_ptr: *mut #jni::sys::JNIEnv = env.get_raw();
				let interface: *const #jni::sys::JNINativeInterface_ = *env_ptr;
				((*interface).v1_1.#call_fn)(
					env_ptr,
					#this_or_class.as_raw(),
					#field_id,
					jni_value
				);
			}

			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			Ok(())
		}
	}
}
