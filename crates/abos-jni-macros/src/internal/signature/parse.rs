use super::*;

impl Parse for SignatureInput {
	fn parse(input: ParseStream) -> Result<Self> {
		// Parse properties in the form: prop = value, prop = value, ...
		// If an argument is unnamed, it's assumed to be the signature.
		// This allows properties to be specified in any order and makes it easy
		// for wrapper macros to inject properties like `jni = foo,` without
		// needing to parse the syntax or worry about positioning.

		let mut signature: Option<Signature> = None;

		let jni_path = crate::internal::utils::parse_jni_crate_override(&input)?;

		// Initialize TypeMappings unconditionally now that we have jni_crate
		let mut type_mappings = TypeMappings::new(&jni_path);

		// Parse remaining properties/arguments
		while !input.is_empty() {
			// Check if this looks like a named property ('<ident> = <value>' or '<ident> { ... }')
			let is_named_property = {
				let fork = input.fork();
				if fork.peek(Ident) {
					if let Ok(_ident) = fork.parse::<Ident>() {
						fork.peek(Token![=]) || fork.peek(syn::token::Brace)
					} else {
						false
					}
				} else {
					false
				}
			};

			if is_named_property {
				// Parse named property
				let prop_name = input.parse::<Ident>()?;
				let prop_str = prop_name.to_string();

				match prop_str.as_str() {
					"jni" => {
						// jni can only be the first property
						return Err(syn::Error::new(
							prop_name.span(),
							"jni property must be the first property if specified",
						));
					}
					"sig" => {
						input.parse::<Token![=]>()?;
						if signature.is_some() {
							return Err(syn::Error::new(
								prop_name.span(),
								"signature specified multiple times (either as unnamed argument or with 'sig =')",
							));
						}
						signature = Some(parse_signature(input, &type_mappings)?);
					}
					"type_map" => {
						type_mappings.parse_mappings(input)?;
					}
					_ => {
						return Err(syn::Error::new(
							prop_name.span(),
							format!(
								"unknown property '{}'. Valid properties are: sig, type_map, jni",
								prop_str
							),
						));
					}
				}
			} else {
				// Unnamed argument - assume it's the signature
				if signature.is_some() {
					return Err(syn::Error::new(
						input.span(),
						"signature specified multiple times (either as unnamed argument or with 'sig =')",
					));
				}
				signature = Some(parse_signature(input, &type_mappings)?);
			}

			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}

		// Signature is required
		let signature = signature.ok_or_else(|| {
			syn::Error::new(
				Span::call_site(),
				"signature is required (either as unnamed argument or with 'sig =')",
			)
		})?;

		Ok(SignatureInput {
			signature,
			type_mappings,
		})
	}
}

/// Parse a signature (method or field) from input
pub(super) fn parse_signature(input: ParseStream, type_mappings: &TypeMappings) -> Result<Signature> {
	// If the input starts with a string literal, it could either be a raw JNI signature
	// or a Java class name for a field signature.
	//
	// If the string literal contains dots (.), we'll treat it as a Java class name for a field signature.
	if input.peek(syn::LitStr) || input.peek(syn::LitCStr) {
		// Parse the string literal
		let raw_sig: String = if input.peek(syn::LitCStr) {
			let lit = input.parse::<syn::LitCStr>()?;
			let cstr = lit.value();
			cstr.to_str()
				.map_err(|e| syn::Error::new(lit.span(), format!("CStr literal must contain valid UTF-8: {}", e)))?
				.to_string()
		} else {
			let lit = input.parse::<syn::LitStr>()?;
			lit.value()
		};

		let is_raw_jni_sig = !raw_sig.contains('.');

		if is_raw_jni_sig {
			// Parse as raw JNI signature
			parse_raw_jni_signature(&raw_sig)
		} else {
			// It contains a dot, so treat it as a Java class name for a field signature
			// Parse it as a JavaClassName
			let parts: Vec<&str> = raw_sig.split('.').collect();
			if parts.is_empty() {
				return Err(syn::Error::new(
					input.span(),
					format!("Empty Java class name in string literal: '{}'", raw_sig),
				));
			}

			let class = parts.last().unwrap().to_string();
			let package: Vec<String> = parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect();

			let java_class = JavaClassName { package, class };
			Ok(Signature::Field(FieldSignature {
				field_type: SigType::Object(java_class),
			}))
		}
	} else if input.peek(token::Paren) {
		// Method signature: (args) [-> ret]
		let args_content;
		parenthesized!(args_content in input);

		let mut parameters = Vec::new();

		if !args_content.is_empty() {
			let mut param_index = 0;
			loop {
				let param = parse_parameter_with_index(&args_content, param_index, type_mappings)?;
				parameters.push(param);
				param_index += 1;

				if args_content.is_empty() {
					break;
				}
				args_content.parse::<Token![,]>()?;
				if args_content.is_empty() {
					break;
				}
			}
		}

		let return_type = if input.peek(Token![->]) {
			input.parse::<Token![->]>()?;

			parse_type(input, type_mappings)?
		} else {
			// Default return type is void
			SigType::Alias("void".to_string())
		};

		Ok(Signature::Method(MethodSignature {
			parameters,
			return_type,
		}))
	} else {
		// Field signature: just a type
		let field_type = parse_type(input, type_mappings)?;
		Ok(Signature::Field(FieldSignature { field_type }))
	}
}

/// Parse a raw JNI signature string into a Signature
/// This parses strings like "(ILjava/lang/String;)V" or "[I" or "Ljava/lang/String;"
pub(super) fn parse_raw_jni_signature(sig: &str) -> Result<Signature> {
	// Check if it's a method signature (starts with '(')
	if sig.starts_with('(') {
		parse_raw_jni_method_signature(sig)
	} else {
		parse_raw_jni_field_signature(sig)
	}
}

/// Parse a raw JNI method signature string
pub(super) fn parse_raw_jni_method_signature(sig: &str) -> Result<Signature> {
	if !sig.starts_with('(') {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Method signature must start with '(': '{}'", sig),
		));
	}

	// Find the closing ')'
	let close_paren = sig.find(')').ok_or_else(|| {
		syn::Error::new(
			Span::call_site(),
			format!("Method signature missing closing ')': '{}'", sig),
		)
	})?;

	// Parse arguments
	let args_str = &sig[1..close_paren];
	let mut args = Vec::new();
	let mut i = 0;
	let args_bytes = args_str.as_bytes();

	while i < args_bytes.len() {
		let (java_type, consumed) = parse_raw_jni_type(&args_str[i..])?;
		args.push(java_type);
		i += consumed;
	}

	// Parse return type
	let ret_str = &sig[close_paren + 1..];
	if ret_str.is_empty() {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Method signature missing return type: '{}'", sig),
		));
	}

	let (ret, consumed) = parse_raw_jni_type(ret_str)?;
	if consumed != ret_str.len() {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Trailing input: '{}' while parsing '{}'", &ret_str[consumed..], sig),
		));
	}

	// Create dummy parameters with generated names
	let parameters = args
		.into_iter()
		.enumerate()
		.map(|(i, ty)| Parameter {
			name: Ident::new(&format!("arg{}", i), Span::call_site()),
			ty,
		})
		.collect();

	Ok(Signature::Method(MethodSignature {
		parameters,
		return_type: ret,
	}))
}

/// Parse a raw JNI field signature string
pub(super) fn parse_raw_jni_field_signature(sig: &str) -> Result<Signature> {
	let (field_type, consumed) = parse_raw_jni_type(sig)?;
	if consumed != sig.len() {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Trailing input: '{}' while parsing '{}'", &sig[consumed..], sig),
		));
	}

	// Validate that void cannot be used as a field type
	if matches!(field_type, SigType::Alias(ref name) if name == "void") {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("void cannot be used as a field type in signature: '{}'", sig),
		));
	}

	Ok(Signature::Field(FieldSignature { field_type }))
}

/// Parse a single JNI type descriptor from a string
/// Returns the JavaType and the number of bytes consumed
pub(super) fn parse_raw_jni_type(s: &str) -> Result<(SigType, usize)> {
	let bytes = s.as_bytes();
	if bytes.is_empty() {
		return Err(syn::Error::new(
			Span::call_site(),
			"Expected type descriptor but found empty string",
		));
	}

	match bytes[0] as char {
		'Z' => Ok((SigType::Alias("jboolean".to_string()), 1)),
		'B' => Ok((SigType::Alias("jbyte".to_string()), 1)),
		'C' => Ok((SigType::Alias("jchar".to_string()), 1)),
		'S' => Ok((SigType::Alias("jshort".to_string()), 1)),
		'I' => Ok((SigType::Alias("jint".to_string()), 1)),
		'J' => Ok((SigType::Alias("jlong".to_string()), 1)),
		'F' => Ok((SigType::Alias("jfloat".to_string()), 1)),
		'D' => Ok((SigType::Alias("jdouble".to_string()), 1)),
		'V' => Ok((SigType::Alias("void".to_string()), 1)),
		'[' => {
			// Array type - parse element type recursively
			let (elem_type, elem_consumed) = parse_raw_jni_type(&s[1..])?;

			// Validate that void cannot be used as an array element type
			if matches!(elem_type, SigType::Alias(ref name) if name == "void") {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("void cannot be used as an array element type in signature: '{}'", s),
				));
			}

			Ok((SigType::Array(Box::new(elem_type), 1), 1 + elem_consumed))
		}
		'L' => {
			// Object type - find the semicolon
			let end = s.find(';').ok_or_else(|| {
				syn::Error::new(Span::call_site(), format!("Object type missing closing ';': '{}'", s))
			})?;

			let class_name = &s[1..end];

			// Validate class name - should not be empty and should not start/end with '/'
			if class_name.is_empty() {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Empty class name in object type: '{}'", s),
				));
			}
			if class_name.starts_with('/') {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name cannot start with '/': '{}'", s),
				));
			}
			if class_name.ends_with('/') {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name cannot end with '/': '{}'", s),
				));
			}

			// Validate that name segments are non-empty
			if class_name.contains("//") {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name contains empty segment: '{}'", s),
				));
			}

			// Parse the class name (could be "java/lang/String" or "java/lang/Outer$Inner")
			// Split by '/' for package segments
			let parts: Vec<&str> = class_name.split('/').collect();

			// The last part is the class name (may contain '$' for inner classes)
			let class = parts.last().unwrap().to_string();

			// Everything before the last part is the package
			let package: Vec<String> = parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect();

			let java_class = JavaClassName { package, class };

			Ok((SigType::Object(java_class), end + 1))
		}
		c => Err(syn::Error::new(
			Span::call_site(),
			format!("Invalid type descriptor: '{}'", c),
		)),
	}
}
