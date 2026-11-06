use super::{
	AbiCheck, BindClassInput, Ident, IdentExt, IsInstanceOfEntry, JavaClassName, LitBool, Parse, ParseStream, Result,
	Token, TypeMappings, abi_check, api, braced, constructors, fields, hooks, is_instance_of, java_type, methods,
	native_methods, native_methods_catch_unwind, native_methods_error_policy, native_methods_export, native_trait,
	parse_constructors, parse_fields, parse_methods, parse_native_methods, parse_visibility, priv_type, quote,
	rust_type, rust_type_vis, type_map,
};

impl Parse for BindClassInput {
	fn parse(input: ParseStream) -> Result<Self> {
		let jni_crate = crate::internal::utils::parse_jni_crate_override(&input)?;

		let mut type_mappings = TypeMappings::new(&jni_crate);

		// Format:
		//
		// ```
		// #[attributes]
		// key = value,
		// #[attributes]
		// key = { ... },
		// #[attributes]
		// key { ... },
		// #[attributes]
		// RustType => java.Type,
		// ```

		// Initialize all possible properties
		let mut type_visibility = None;
		let mut type_name_opt = None;
		let mut type_attrs = Vec::new();
		let mut java_class_opt = None;
		let mut api_name = None;
		let mut priv_type = None;
		let mut native_trait_name = None;
		let mut is_instance_of = Vec::new();
		let mut constructors = Vec::new();
		let mut methods = Vec::new();
		let mut fields = Vec::new();
		let mut native_methods = Vec::new();
		let mut load_class_closure = None;
		let mut init_priv_closure = None;
		let mut native_methods_export = true;
		let mut native_methods_catch_unwind = true;
		let mut abi_check = AbiCheck::default();
		let mut native_methods_error_policy = None;
		let mut jni_core = false;
		let mut sys_type = None;

		while !input.is_empty() {
			let mut prop_attrs = input.call(syn::Attribute::parse_outer)?;

			// Try to peek for <ident> = pattern or <ident> { ... }
			// (anything else is treated as `[vis] RustType => java.Type` shorthand)
			let fork = input.fork();
			let is_prop = if let Ok(_ident) = fork.call(Ident::parse_any) {
				// Note: we need to rule out `=>` before checking for `=` otherwise we could split the `=>` token
				// and misinterpret the shorthand syntax as a property
				!fork.peek(Token![=>]) && (fork.peek(Token![=]) || fork.peek(syn::token::Brace))
			} else {
				false
			};

			if is_prop {
				let lookahead = input.lookahead1();

				if lookahead.peek(rust_type) {
					let prop_ident: Ident = input.parse()?;
					if type_name_opt.is_some() {
						return Err(syn::Error::new(prop_ident.span(), "Rust type name already specified"));
					}
					input.parse::<Token![=]>()?;

					if let Some(vis) = parse_visibility(input)? {
						type_visibility = Some(vis);
					}

					type_name_opt = Some(input.parse()?);
					type_attrs = std::mem::take(&mut prop_attrs);
				} else if lookahead.peek(rust_type_vis) {
					let prop_ident: Ident = input.parse()?;
					if type_visibility.is_some() {
						return Err(syn::Error::new(prop_ident.span(), "Type visibility already specified"));
					}
					input.parse::<Token![=]>()?;
					let Some(vis) = parse_visibility(input)? else {
						return Err(syn::Error::new(
							prop_ident.span(),
							"Expected visibility specifier after 'type_visibility ='",
						));
					};
					type_visibility = Some(vis);
				} else if lookahead.peek(java_type) {
					let prop_ident: Ident = input.parse()?;
					if java_class_opt.is_some() {
						return Err(syn::Error::new(prop_ident.span(), "Java class name already specified"));
					}
					input.parse::<Token![=]>()?;
					java_class_opt = Some(input.parse()?);
				} else if lookahead.peek(api) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					api_name = Some(input.parse()?);
				} else if lookahead.peek(self::priv_type) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					priv_type = Some(input.parse()?);
				} else if lookahead.peek(native_trait) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					native_trait_name = Some(input.parse()?);
				} else if lookahead.peek(self::is_instance_of) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let is_instance_content;
					braced!(is_instance_content in input);

					while !is_instance_content.is_empty() {
						// Try to parse stem = Type or stem: Type or just Type
						let stem: Option<String>;
						let type_path: syn::Path;

						// Check if we have a simple identifier (not a path) followed by = or :
						// We need to distinguish between:
						//   - "stem = Path" or "stem: Path" (has stem)
						//   - "path::to::Type" (no stem, just a path)
						// To do this, check if the next token after an identifier is = or : (not ::)
						let has_stem = is_instance_content.peek(Ident)
							&& !is_instance_content.peek2(Token![::])
							&& (is_instance_content.peek2(Token![=]) || is_instance_content.peek2(Token![:]));

						if has_stem {
							let stem_ident: Ident = is_instance_content.parse()?;
							if is_instance_content.peek(Token![=]) {
								is_instance_content.parse::<Token![=]>()?;
							} else {
								is_instance_content.parse::<Token![:]>()?;
							}
							type_path = is_instance_content.parse()?;
							stem = Some(stem_ident.to_string());
						} else {
							// Just a bare type path
							type_path = is_instance_content.parse()?;
							stem = None;
						}

						let type_path_str = quote!(#type_path).to_string().replace(' ', "");

						// Validate that JObject is not explicitly specified
						if type_path_str == "JObject" || type_path_str == "abos_jni::objects::JObject" {
							return Err(syn::Error::new_spanned(
								&type_path,
								"JObject should not be explicitly specified in is_instance_of - all types are already instances of JObject",
							));
						}

						is_instance_of.push(IsInstanceOfEntry {
							type_alias: type_path_str,
							stem,
						});

						// Require comma between entries, but trailing comma is optional
						if !is_instance_content.is_empty() {
							is_instance_content.parse::<Token![,]>()?;
						}
					}
				} else if lookahead.peek(self::type_map) {
					let _ = input.parse::<Ident>()?;
					type_mappings.parse_mappings(input)?;
				} else if lookahead.peek(self::hooks) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let hooks_content;
					braced!(hooks_content in input);

					while !hooks_content.is_empty() {
						let hooks_key = hooks_content.parse::<Ident>()?;
						hooks_content.parse::<Token![=]>()?;

						// Parse closure as an expression until we hit a comma
						let closure_expr: syn::Expr = hooks_content.parse()?;
						let closure_tokens = quote! { #closure_expr };

						match hooks_key.to_string().as_str() {
							"load_class" => {
								load_class_closure = Some(closure_tokens);
							}
							"init_priv" => {
								init_priv_closure = Some(closure_tokens);
							}
							_ => {
								return Err(syn::Error::new(
									hooks_key.span(),
									format!("Unknown hooks property: {hooks_key}"),
								));
							}
						}

						// Require comma between entries, but trailing comma is optional
						if !hooks_content.is_empty() {
							hooks_content.parse::<Token![,]>()?;
						}
					}
				} else if lookahead.peek(self::constructors) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let constructors_content;
					braced!(constructors_content in input);
					constructors = parse_constructors(&constructors_content, &type_mappings)?;
				} else if lookahead.peek(self::methods) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let methods_content;
					braced!(methods_content in input);

					methods.extend(parse_methods(&methods_content, &type_mappings)?);
				} else if lookahead.peek(self::fields) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let fields_content;
					braced!(fields_content in input);

					fields.extend(parse_fields(&fields_content, &type_mappings)?);
				} else if lookahead.peek(self::native_methods) {
					let _ = input.parse::<Ident>()?;
					// Optional '=' before block
					if input.peek(Token![=]) {
						input.parse::<Token![=]>()?;
					}

					let native_methods_content;
					braced!(native_methods_content in input);

					// A native method block can produce a NativeMethod implementation to register /
					// export, as well as a public Method API if any visibility is specified
					let (block_native_methods, block_methods) =
						parse_native_methods(&native_methods_content, &type_mappings)?;
					native_methods.extend(block_native_methods);
					methods.extend(block_methods);
				} else if lookahead.peek(self::native_methods_export) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					let value: LitBool = input.parse()?;
					native_methods_export = value.value();
				} else if lookahead.peek(self::native_methods_error_policy) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					native_methods_error_policy = Some(input.parse::<syn::Path>()?);
				} else if lookahead.peek(self::abi_check) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					abi_check = input.parse::<AbiCheck>()?;
				} else if lookahead.peek(self::native_methods_catch_unwind) {
					let _ = input.parse::<Ident>()?;
					input.parse::<Token![=]>()?;
					let value: LitBool = input.parse()?;
					native_methods_catch_unwind = value.value();
				} else {
					// Private or invalid properties that shouldn't show in in a lookahead1 error
					// as a suggested property

					let property_name: Ident = input.parse()?;
					let property_str = property_name.to_string();

					match property_str.as_str() {
						"jni" => {
							// jni can only be the first property
							return Err(syn::Error::new(
								property_name.span(),
								"jni property must be the first property if specified",
							));
						}
						"__jni_core" => {
							input.parse::<Token![=]>()?;
							let value: LitBool = input.parse()?;
							jni_core = value.value();
						}
						"__sys_type" => {
							input.parse::<Token![=]>()?;
							sys_type = Some(input.parse()?);
						}
						_ => {
							return Err(lookahead.error());
						}
					}
				}
			} else {
				// Expect shorthand syntax: [vis] RustType => java.Type
				type_visibility = parse_visibility(input)?;

				let rust_type: Ident = input.parse()?;
				input.parse::<Token![=>]>()?;
				let java_type: JavaClassName = input.parse()?;

				if type_name_opt.is_some() || java_class_opt.is_some() {
					return Err(syn::Error::new(
						input.span(),
						"The Rust type and Java class can only be specified once",
					));
				}
				type_name_opt = Some(rust_type);
				type_attrs = std::mem::take(&mut prop_attrs);
				java_class_opt = Some(java_type);
			}

			// As soon as we know the type name and java class, we can can register
			// a Self type mapping
			if let (Some(type_name), Some(java_class)) = (&type_name_opt, &java_class_opt) {
				let type_path_str = type_name.to_string();
				// A Self type mapping already exists (e.g. from a type_map block or builtin type)
				// then we let that take precedence. This avoids conflicting with shared type_maps
				// in a project that may also use fully qualified Rust type names that will look
				// like a type mismatch if we call insert_ref_type again here.
				#[allow(clippy::collapsible_if)]
				if !type_mappings.is_core_java_type(java_class) && type_mappings.map_alias(&type_path_str).is_none() {
					// Note: if the type mapping already exists (e.g. from a type_map block or builtin type)
					// this will validate that the type matches the existing mapping
					if let Err(err) = type_mappings.insert_ref_type(
						&type_path_str,
						&type_path_str,
						java_class.clone(),
						false, // implied because the type isn't already mapped
					) {
						return Err(syn::Error::new(
							type_name.span(),
							format!("Failed to register Self type mapping: {err}"),
						));
					}
				}
			}

			// Require comma after each property, except trailing comma is optional
			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}

		// Validate required properties
		let type_name = type_name_opt
			.ok_or_else(|| syn::Error::new(input.span(), "Missing required property: rust_type = RustTypeName"))?;

		let java_class = java_class_opt
			.ok_or_else(|| syn::Error::new(input.span(), "Missing required property: java_type = java.class.Name"))?;

		Ok(Self {
			type_name,
			type_visibility,
			type_attrs,
			java_class,
			api_name,
			priv_type,
			native_trait_name,
			is_instance_of,
			type_mappings,
			constructors,
			methods,
			fields,
			native_methods,
			load_class_closure,
			init_priv_closure,
			native_methods_export,
			native_methods_catch_unwind,
			abi_check,
			native_methods_error_policy,
			jni_core,
			jni_crate,
			sys_type,
		})
	}
}
