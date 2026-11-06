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
	let call_prefix = if is_static { "GetStatic" } else { "Get" };

	// Determine the JNI function name and return value handling based on the field type
	let (top_check, call_fn, return_tokens) = if let Some(prim) = field_type.try_as_primitive(type_mappings) {
		if prim == PrimitiveType::Void {
			return quote! { Ok(()) };
		}
		let call_fn = format_ident!("{}{}Field", call_prefix, prim.jni_name());
		(quote! {}, call_fn, quote! { ret })
	} else {
		// For objects, arrays, and rust references, we call GetObjectField
		let call_fn = format_ident!("{}ObjectField", call_prefix);
		let return_type_tokens = sig_type_to_rust_return_type(field_type, &quote! { 'env_local }, type_mappings, jni);
		(
			quote! { env.assert_top(); },
			call_fn,
			quote! { unsafe { <#return_type_tokens>::from_raw(env, ret) } },
		)
	};

	// The JNI spec asserts that `Get<Type>Field` _must_ not be called with a `null` object reference
	// Note: we assume that the class will never be null when getting static fields
	let null_check = if is_static {
		quote! {}
	} else {
		let error_msg = format!("Attempted to set '{debug_name}' field with a null object");
		quote! {
			if #this_or_class.is_null() {
				return Err(#jni::errors::Error::NullPtr(#error_msg));
			}
		}
	};

	quote! {
		{
			// None of these getter functions are exception safe, so we can't go ahead with the JNI
			// call if there's currently a pending exception.
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
					#field_id
				)
			};

			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}

			Ok(#return_tokens)
		}
	}
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
	let call_prefix = if is_static { "SetStatic" } else { "Set" };

	// Determine the JNI function name and value cast type based on the field type
	let (call_fn, val_cast) = if let Some(prim) = field_type.try_as_primitive(type_mappings) {
		match prim {
			PrimitiveType::Void => {
				// Void fields don't make sense but we handle them for completeness
				return quote! { Ok(()) };
			}
			PrimitiveType::Boolean => {
				let call_fn = format_ident!("{}BooleanField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jboolean })
			}
			PrimitiveType::Byte => {
				let call_fn = format_ident!("{}ByteField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jbyte })
			}
			PrimitiveType::Char => {
				let call_fn = format_ident!("{}CharField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jchar })
			}
			PrimitiveType::Short => {
				let call_fn = format_ident!("{}ShortField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jshort })
			}
			PrimitiveType::Int => {
				let call_fn = format_ident!("{}IntField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jint })
			}
			PrimitiveType::Long => {
				let call_fn = format_ident!("{}LongField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jlong })
			}
			PrimitiveType::Float => {
				let call_fn = format_ident!("{}FloatField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jfloat })
			}
			PrimitiveType::Double => {
				let call_fn = format_ident!("{}DoubleField", call_prefix);
				(call_fn, quote! { #val as #jni::sys::jdouble })
			}
		}
	} else {
		// For objects, arrays, and rust references, we call SetObjectField
		let call_fn = format_ident!("{}ObjectField", call_prefix);
		(call_fn, quote! { #val.as_ref().as_raw() })
	};

	// The JNI spec asserts that `Set<Type>Field` _must_ not be called with a `null` object reference
	// Note: we assume that the class will never be null when setting static fields
	let null_check = if is_static {
		quote! {}
	} else {
		let error_msg = format!("Attempted to set '{debug_name}' field with a null object");
		quote! {
			if #this_or_class.is_null() {
				return Err(#jni::errors::Error::NullPtr(#error_msg));
			}
		}
	};

	// Generate the JNI field setter call
	quote! {
		{
			// None of these setter functions are exception safe, so we can't go ahead with the JNI
			// call if there's currently a pending exception.
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
