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
		let value: TokenStream = if crate::internal::types::is_rust_bool(field_type, type_mappings) {
			quote! { ret != #jni::sys::JNI_FALSE }
		} else {
			quote! { ret }
		};
		(quote! {}, call_fn, value)
	} else {
		let call_fn: proc_macro2::Ident = format_ident!("{}ObjectField", call_prefix);
		let return_type_tokens: TokenStream =
			sig_type_to_rust_return_type(field_type, &quote! { 'env_local }, type_mappings, jni);
		(
			quote! { env.assert_top(); },
			call_fn,
			quote! { unsafe { <#return_type_tokens>::from_raw(env, ret) } },
		)
	};

	// The JNI spec asserts that `Get<Type>Field` _must_ not be called with a `null` object reference
	// Note: we assume that the class will never be null when getting static fields
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

	let (call_fn, val_cast) = if let Some(prim) = field_type.try_as_primitive(type_mappings) {
		match prim {
			PrimitiveType::Void => {
				// Void fields don't make sense but we handle them for completeness
				return quote! { Ok(()) };
			}
			PrimitiveType::Boolean => {
				let call_fn: proc_macro2::Ident = format_ident!("{}BooleanField", call_prefix);
				(call_fn, quote! { #jni::sys::jboolean::from(#val) })
			}
			PrimitiveType::Byte => {
				let call_fn: proc_macro2::Ident = format_ident!("{}ByteField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jbyte })
			}
			PrimitiveType::Char => {
				let call_fn: proc_macro2::Ident = format_ident!("{}CharField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jchar })
			}
			PrimitiveType::Short => {
				let call_fn: proc_macro2::Ident = format_ident!("{}ShortField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jshort })
			}
			PrimitiveType::Int => {
				let call_fn: proc_macro2::Ident = format_ident!("{}IntField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jint })
			}
			PrimitiveType::Long => {
				let call_fn: proc_macro2::Ident = format_ident!("{}LongField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jlong })
			}
			PrimitiveType::Float => {
				let call_fn: proc_macro2::Ident = format_ident!("{}FloatField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jfloat })
			}
			PrimitiveType::Double => {
				let call_fn: proc_macro2::Ident = format_ident!("{}DoubleField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jdouble })
			}
		}
	} else {
		let call_fn: proc_macro2::Ident = format_ident!("{}ObjectField", call_prefix);
		(call_fn, quote! { #val.as_raw() })
	};

	// The JNI spec asserts that `Set<Type>Field` _must_ not be called with a `null` object reference
	// Note: we assume that the class will never be null when setting static fields
	let null_check: TokenStream = super::calls::generate_receiver_null_check(
		is_static,
		this_or_class,
		&format!("Attempted to set '{debug_name}' field with a null object"),
		jni,
	);

	quote! {
		{
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
					#val_cast
				);
			}

			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			Ok(())
		}
	}
}
