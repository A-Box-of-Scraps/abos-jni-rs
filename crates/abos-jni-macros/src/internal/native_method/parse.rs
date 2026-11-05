use super::*;

impl NativeMethodStructInput {
	fn parse(input: ParseStream) -> Result<Self> {
		// Try to detect jni crate path using parse_jni_crate_override
		let jni_crate = crate::internal::utils::parse_jni_crate_override(&input)?;

		let mut rust_type_opt = None;
		let mut java_type_opt = None;
		let mut type_mappings = TypeMappings::new(&jni_crate);
		let mut error_policy_ident = None;

		let mut is_static_fn = false;
		let mut is_raw_fn = false;
		let mut export_opt = NativeMethodExport::No;
		let mut abi_check = AbiCheck::default();
		let mut catch_unwind = None;

		let mut java_method_name = None;
		let mut method_signature = None;
		let mut inline_fn_path = None;
		let mut fn_path = None;

		// Parse properties and inline signature
		while !input.is_empty() {
			// Try to peek for <ident> = pattern or <ident> { ... }
			// (anything else is treated as inline signature)
			let fork = input.fork();
			let is_prop = if let Ok(_ident) = fork.call(Ident::parse_any) {
				fork.peek(Token![=]) || fork.peek(syn::token::Brace)
			} else {
				false
			};

			if is_prop {
				// Parse as property: <ident> = <value> or <ident> { ... }
				let lookahead = input.lookahead1();

				if lookahead.peek(rust_type) {
					// Parse: rust_type = Type
					input.parse::<rust_type>()?;
					input.parse::<Token![=]>()?;
					rust_type_opt = Some(input.parse::<syn::Path>()?);
				} else if lookahead.peek(java_type) {
					// Parse: java_type = "com.example.MyClass"
					input.parse::<java_type>()?;
					input.parse::<Token![=]>()?;
					let class_name = input.parse::<JavaClassName>()?;
					java_type_opt = Some(class_name);
				} else if lookahead.peek(name) {
					// Parse: name = "methodName"
					input.parse::<name>()?;
					input.parse::<Token![=]>()?;
					let lit = input.parse::<syn::LitStr>()?;
					java_method_name = Some(lit.value());
				} else if lookahead.peek(sig) {
					// Parse: sig = (args) -> ret
					method_signature = Some(parse_method_sig(input, &type_mappings)?);
				} else if lookahead.peek(Token![static]) {
					// Parse: static = true/false
					input.parse::<Token![static]>()?;
					input.parse::<Token![=]>()?;
					let lit = input.parse::<syn::LitBool>()?;
					is_static_fn = lit.value();
				} else if lookahead.peek(raw) {
					// Parse: raw = true/false
					input.parse::<raw>()?;
					input.parse::<Token![=]>()?;
					let lit = input.parse::<syn::LitBool>()?;
					is_raw_fn = lit.value();
				} else if lookahead.peek(export) {
					// Parse: export = true/false/"name"
					input.parse::<export>()?;
					input.parse::<Token![=]>()?;
					if input.peek(syn::LitStr) {
						// export = "customName"
						let name = input.parse::<syn::LitStr>()?.value();
						export_opt = NativeMethodExport::WithName(name);
					} else if input.peek(syn::LitBool) {
						// export = true or export = false
						let lit_bool = input.parse::<syn::LitBool>()?;
						export_opt = if lit_bool.value() {
							NativeMethodExport::WithAutoMangle
						} else {
							NativeMethodExport::No
						};
					} else {
						return Err(syn::Error::new(
							input.span(),
							"export must be 'true', 'false', or a string literal",
						));
					}
				} else if lookahead.peek(self::abi_check) {
					// Parse: abi_check = Always | UnsafeNever | UnsafeDebugOnly
					input.parse::<self::abi_check>()?;
					input.parse::<Token![=]>()?;
					abi_check = input.parse::<AbiCheck>()?;
				} else if lookahead.peek(self::catch_unwind) {
					// Parse: catch_unwind = true/false
					input.parse::<self::catch_unwind>()?;
					input.parse::<Token![=]>()?;
					let lit = input.parse::<syn::LitBool>()?;
					catch_unwind = Some(lit.value());
				} else if lookahead.peek(Token![fn]) {
					// Parse: fn = path_to_fn
					input.parse::<Token![fn]>()?;
					input.parse::<Token![=]>()?;

					fn_path = Some(input.parse::<syn::Path>()?);
				} else if lookahead.peek(error_policy) {
					// Parse: error_policy = ErrorPolicyIdent
					input.parse::<error_policy>()?;
					input.parse::<Token![=]>()?;
					error_policy_ident = Some(input.parse::<Ident>()?);
				} else if lookahead.peek(type_map) {
					// Parse: type_map = { ... }
					input.parse::<type_map>()?;
					type_mappings.parse_mappings(input)?;
				} else {
					return Err(lookahead.error());
				}
			} else {
				// Parse inline signature: [static] [raw] [extern] fn [Path::][method](args) -> ret

				// Parse qualifiers in any order (static, raw, extern)
				loop {
					if input.peek(Token![static]) {
						input.parse::<Token![static]>()?;
						is_static_fn = true;
					} else if input.peek(Token![extern]) {
						input.parse::<Token![extern]>()?;
						// give export = true | "name" higher precedence
						if matches!(export_opt, NativeMethodExport::No) {
							export_opt = NativeMethodExport::WithAutoMangle;
						}
					} else if input.peek(raw) {
						input.parse::<raw>()?;
						is_raw_fn = true;
					} else {
						break;
					}
				}

				// Require 'fn' keyword for both shorthand and block syntax
				if !input.peek(Token![fn]) {
					return Err(syn::Error::new(
						input.span(),
						"Expected 'fn' keyword before RustType::method_name",
					));
				}
				input.parse::<Token![fn]>()?;

				// Try to parse a path followed by parentheses
				let full_path = input.parse::<syn::Path>()?;
				inline_fn_path = Some(full_path.clone());

				// Check if this path has multiple segments (Type::method pattern)
				let segments: Vec<_> = full_path.segments.iter().collect();

				let (type_path_opt, method_name_from_path) = if segments.len() >= 2 {
					// Split into type path and method name
					let method_ident = &segments[segments.len() - 1].ident;
					let method_name_snake = method_ident.to_string();

					// Build the type path (everything except the last segment)
					let type_segments = &segments[..segments.len() - 1];
					let mut type_path = syn::Path {
						leading_colon: full_path.leading_colon,
						segments: syn::punctuated::Punctuated::new(),
					};
					for seg in type_segments {
						type_path.segments.push((*seg).clone());
					}

					(Some(type_path), method_name_snake)
				} else {
					// Single segment - just a method name
					(None, segments[0].ident.to_string())
				};

				// Parse method signature: (args) -> ret
				let args_content;
				syn::parenthesized!(args_content in input);

				let mut parameters = Vec::new();
				let mut param_index = 0;

				while !args_content.is_empty() {
					let param = parse_parameter_with_index(&args_content, param_index, &type_mappings)?;
					parameters.push(param);
					param_index += 1;

					if !args_content.is_empty() {
						args_content.parse::<Token![,]>()?;
					}
				}

				let return_type = if input.peek(Token![->]) {
					input.parse::<Token![->]>()?;
					parse_type(input, &type_mappings)?
				} else {
					// No return type means this is constructor shorthand - use void
					SigType::Alias("void".to_string())
				};

				method_signature = Some(MethodSignature {
					parameters,
					return_type,
				});

				// Set values from parsed signature
				if type_path_opt.is_some() && rust_type_opt.is_none() {
					rust_type_opt = type_path_opt;
				}

				if java_method_name.is_none() {
					// Convert snake_case to lowerCamelCase for Java method name
					java_method_name = Some(snake_case_to_lower_camel_case(&method_name_from_path));
				}

				// fn = <path> takes precedence over the default RustType::method_name path
				if fn_path.is_none() {
					fn_path = full_path.clone().into();
				} else if fn_path.is_none() {
					// Default to the full path if no => was given
					fn_path = Some(full_path.clone());
				}
			}

			// Optional trailing comma
			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}

		// Validate required properties
		let method_name = java_method_name.ok_or_else(|| {
            syn::Error::new(
                input.span(),
                "Missing method name (use 'name = \"javaCamelCaseName\"' or inline signature like 'fn RustType::snake_case_method_name()')",
            )
        })?;
		let method_signature = method_signature.ok_or_else(|| {
            syn::Error::new(
                input.span(),
                "Missing method signature (use 'sig = (args...) -> ret' or inline signature like 'fn RustType::method_name(args...) -> ret')",
            )
        })?;
		// fn = <path> takes precedence over the default RustType::method_name path from an inline signature
		let fn_path = if let Some(fn_path) = fn_path {
			fn_path
		} else {
			if inline_fn_path.is_none() {
				return Err(syn::Error::new(
					input.span(),
					"Missing 'fn' and can't infer a default function path without an inline signature like 'fn RustType::method_name(args..) -> ret'.",
				));
			}
			inline_fn_path.clone().unwrap()
		};

		// Validate: java_type required if export = true
		if matches!(export_opt, NativeMethodExport::WithAutoMangle) && java_type_opt.is_none() {
			return Err(syn::Error::new(
				input.span(),
				"java_type = \"...\" is required when 'export = true' or 'extern' qualifier is used in signature",
			));
		}

		// Validate that error / error_policy / catch_unwind are not specified with raw = true
		if is_raw_fn {
			if error_policy_ident.is_some() {
				return Err(syn::Error::new(
					input.span(),
					"Cannot specify 'error_policy' when using 'raw = true' or 'raw' in signature - error_policy only applies to wrapped functions",
				));
			}
			if catch_unwind.is_some() {
				return Err(syn::Error::new(
					input.span(),
					"Cannot specify 'catch_unwind' when using 'raw = true' or 'raw' in signature - catch_unwind only applies to wrapped functions",
				));
			}
		}

		Ok(NativeMethodStructInput {
			is_static: is_static_fn,
			jni_crate,
			rust_type: rust_type_opt,
			java_type: java_type_opt,
			java_method_name: method_name,
			method_signature,
			fn_path,
			type_mappings,
			is_raw_fn,
			error_policy: error_policy_ident,
			export: export_opt,
			abi_check,
			catch_unwind: catch_unwind.unwrap_or(true),
		})
	}
}

impl Parse for NativeMethodStructInput {
	fn parse(input: ParseStream) -> Result<Self> {
		Self::parse(input)
	}
}
