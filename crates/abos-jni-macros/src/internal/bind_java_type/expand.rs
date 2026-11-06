use super::{
	BindClassInput, Result, Span, TokenStream, VisibilitySpec, format_ident, generate_api_get_method,
	generate_api_struct, generate_base_methods, generate_constructor_method_ids, generate_constructors,
	generate_field_ids, generate_fields, generate_is_instance_of_code, generate_method_ids, generate_methods,
	generate_native_methods_code, generate_reference_impl, generate_type_struct, quote,
};

/// Generate code for the `bind_java_type` macro
pub fn bind_java_type_impl(input: TokenStream) -> Result<TokenStream> {
	let input: BindClassInput = syn::parse2(input)?;

	let type_name = &input.type_name;
	let java_class = &input.java_class;
	let java_class_internal = java_class.to_jni_internal();
	let java_class_dotted = java_class.to_java_dotted();
	let jni = &input.jni_crate;

	// Check if this is a core Java type that cannot be bound without __jni_core = true
	if input.type_mappings.is_core_java_type(java_class) && !input.jni_core {
		return Err(syn::Error::new(
			Span::call_site(),
			format!(
				"Cannot create binding for core Java type '{java_class_dotted}'. Core types (java.lang.Object, java.lang.Class, java.lang.Throwable, and java.lang.String) are already provided by the jni crate and cannot be re-bound."
			),
		));
	}

	// Generate API struct name
	let api_name = input.api_name.unwrap_or_else(|| format_ident!("{}API", type_name));

	let type_visibility = input.type_visibility.unwrap_or(VisibilitySpec::PubSelf);

	// Generate the type struct
	let type_struct = generate_type_struct(type_name, &type_visibility, &input.type_attrs, &java_class_dotted, jni);

	// Generate constructor method ID fields and initialization code
	let (constructor_method_id_fields, constructor_method_id_inits) =
		generate_constructor_method_ids(&input.constructors, &input.type_mappings, jni)?;

	// Generate method ID fields and initialization code for methods
	let (method_id_fields, method_id_inits) = generate_method_ids(&input.methods, &input.type_mappings, jni)?;

	// Generate field ID fields and initialization code for fields
	let (field_id_fields, field_id_inits) = generate_field_ids(&input.fields, &input.type_mappings, jni)?;

	// Combine all ID fields and inits
	let all_id_fields = [constructor_method_id_fields, method_id_fields, field_id_fields].concat();

	let all_id_inits = [constructor_method_id_inits, method_id_inits, field_id_inits].concat();

	// Generate the API struct
	let api_struct = generate_api_struct(
		&api_name,
		&type_visibility,
		input.priv_type.as_ref(),
		&all_id_fields,
		jni,
	);

	// Generate native methods support - get registration code for API::get()
	let (native_trait_and_wrappers, native_registration_code) = generate_native_methods_code(
		type_name,
		&api_name,
		&type_visibility,
		&java_class_internal,
		&java_class_dotted,
		&input.native_methods,
		&input.native_trait_name,
		&input.type_mappings,
		input.native_methods_export,
		input.native_methods_catch_unwind,
		input.abi_check,
		&input.native_methods_error_policy,
		jni,
	)?;

	// Generate the API::get() method with native registration integrated
	let api_get_method = generate_api_get_method(
		&api_name,
		type_name,
		&java_class_dotted,
		input.load_class_closure.as_ref(),
		input.init_priv_closure.as_ref(),
		input.priv_type.as_ref(),
		&all_id_inits,
		&native_registration_code,
		&input.is_instance_of,
		&input.type_mappings,
		input.abi_check,
		jni,
	)?;

	// Generate Reference trait implementation
	let reference_impl = generate_reference_impl(type_name, &api_name, &java_class_dotted, jni);

	// Generate base methods (from_raw, null, into_raw)
	let base_methods = generate_base_methods(type_name, input.sys_type.as_ref(), jni);

	// Generate is_instance_of methods and From impls
	let is_instance_of_code =
		generate_is_instance_of_code(type_name, &input.is_instance_of, &input.type_mappings, jni)?;

	// Generate constructor implementations
	let constructors_code =
		generate_constructors(type_name, &api_name, &input.constructors, &input.type_mappings, jni)?;

	// Generate method implementations
	let methods_impl = generate_methods(&api_name, &input.methods, &input.type_mappings, jni)?;

	// Generate field implementations
	let fields_impl = generate_fields(&api_name, &input.fields, &input.type_mappings, jni);

	// Wrap instance methods and fields in impl<'local> block
	let instance_impl = quote! {
		impl<'local> #type_name<'local> {
			#methods_impl
			#fields_impl
		}
	};

	Ok(quote! {
		#type_struct
		#api_struct
		#api_get_method
		#reference_impl
		#base_methods
		#is_instance_of_code
		#constructors_code
		#instance_impl
		#native_trait_and_wrappers
	})
}
