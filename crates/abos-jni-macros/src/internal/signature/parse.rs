use super::{
	FieldSignature, Ident, JavaClassName, MethodSignature, Parameter, Parse, ParseStream, Result, SigType, Signature,
	SignatureInput, Span, Token, TypeMappings, parenthesized, parse_parameter_with_index, parse_type, token,
};

impl Parse for SignatureInput {
	fn parse(input: ParseStream) -> Result<Self> {
		// After the optional jni property, named properties and the signature may appear
		// in any order, allowing wrapper macros to inject properties without parsing them.

		let mut signature: Option<Signature> = None;

		let jni_path: syn::Path = crate::internal::utils::parse_jni_crate_override(input)?;

		let mut type_mappings = TypeMappings::new(&jni_path);

		while !input.is_empty() {
			parse_input_entry(input, &mut signature, &mut type_mappings)?;

			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}

		let signature: Signature = signature.ok_or_else(|| {
			syn::Error::new(
				Span::call_site(),
				"signature is required (either as unnamed argument or with 'sig =')",
			)
		})?;

		Ok(Self {
			signature,
			type_mappings,
		})
	}
}

fn parse_input_entry(
	input: ParseStream,
	signature: &mut Option<Signature>,
	type_mappings: &mut TypeMappings,
) -> Result<()> {
	let is_named_property: bool = {
		let fork: syn::parse::ParseBuffer<'_> = input.fork();
		if fork.peek(Ident) {
			fork.parse::<Ident>()
				.is_ok_and(|_ident| fork.peek(Token![=]) || fork.peek(syn::token::Brace))
		} else {
			false
		}
	};

	if is_named_property {
		let prop_name: Ident = input.parse::<Ident>()?;
		let prop_str: String = prop_name.to_string();

		match prop_str.as_str() {
			"jni" => {
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
				*signature = Some(parse_signature(input, type_mappings)?);
			}
			"type_map" => {
				type_mappings.parse_mappings(input)?;
			}
			_ => {
				return Err(syn::Error::new(
					prop_name.span(),
					format!("unknown property '{prop_str}'. Valid properties are: sig, type_map, jni"),
				));
			}
		}
	} else {
		if signature.is_some() {
			return Err(syn::Error::new(
				input.span(),
				"signature specified multiple times (either as unnamed argument or with 'sig =')",
			));
		}
		*signature = Some(parse_signature(input, type_mappings)?);
	}
	Ok(())
}

/// Parse a signature (method or field) from input
pub(super) fn parse_signature(input: ParseStream, type_mappings: &TypeMappings) -> Result<Signature> {
	// A dot distinguishes a Java field type name from a raw JNI signature.
	if input.peek(syn::LitStr) || input.peek(syn::LitCStr) {
		let raw_sig: String = if input.peek(syn::LitCStr) {
			let lit: syn::LitCStr = input.parse::<syn::LitCStr>()?;
			let cstr: std::ffi::CString = lit.value();
			cstr.to_str()
				.map_err(|e| syn::Error::new(lit.span(), format!("CStr literal must contain valid UTF-8: {e}")))?
				.to_string()
		} else {
			let lit: syn::LitStr = input.parse::<syn::LitStr>()?;
			lit.value()
		};

		let is_raw_jni_sig: bool = !raw_sig.contains('.');

		if is_raw_jni_sig {
			parse_raw_jni_signature(&raw_sig)
		} else {
			let parts: Vec<&str> = raw_sig.split('.').collect();
			if parts.is_empty() {
				return Err(syn::Error::new(
					input.span(),
					format!("Empty Java class name in string literal: '{raw_sig}'"),
				));
			}

			let class: String = parts
				.last()
				.expect("splitting a class name yields at least one component")
				.to_string();
			let package: Vec<String> = parts[..parts.len() - 1]
				.iter()
				.map(std::string::ToString::to_string)
				.collect();

			let java_class = JavaClassName { package, class };
			Ok(Signature::Field(FieldSignature {
				field_type: SigType::Object(java_class),
			}))
		}
	} else if input.peek(token::Paren) {
		let args_content;
		parenthesized!(args_content in input);

		let mut parameters: Vec<Parameter> = Vec::new();

		while !args_content.is_empty() {
			parameters.push(parse_parameter_with_index(
				&args_content,
				parameters.len(),
				type_mappings,
			)?);
			if !args_content.is_empty() {
				args_content.parse::<Token![,]>()?;
			}
		}

		let return_type: SigType = if input.peek(Token![->]) {
			input.parse::<Token![->]>()?;

			parse_type(input, type_mappings)?
		} else {
			SigType::Alias("void".to_string())
		};

		Ok(Signature::Method(MethodSignature {
			parameters,
			return_type,
		}))
	} else {
		let field_type: SigType = parse_type(input, type_mappings)?;
		Ok(Signature::Field(FieldSignature { field_type }))
	}
}

/// Parse a raw JNI signature string into a Signature
/// This parses strings like "(ILjava/lang/String;)V" or "[I" or "Ljava/lang/String;"
pub(super) fn parse_raw_jni_signature(sig: &str) -> Result<Signature> {
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
			format!("Method signature must start with '(': '{sig}'"),
		));
	}

	let close_paren: usize = sig.find(')').ok_or_else(|| {
		syn::Error::new(
			Span::call_site(),
			format!("Method signature missing closing ')': '{sig}'"),
		)
	})?;

	let args_str: &str = &sig[1..close_paren];
	let mut args: Vec<SigType> = Vec::new();
	let mut i = 0;
	let args_bytes: &[u8] = args_str.as_bytes();

	while i < args_bytes.len() {
		let (java_type, consumed) = parse_raw_jni_type(&args_str[i..])?;
		args.push(java_type);
		i += consumed;
	}

	let ret_str: &str = &sig[close_paren + 1..];
	if ret_str.is_empty() {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Method signature missing return type: '{sig}'"),
		));
	}

	let (ret, consumed) = parse_raw_jni_type(ret_str)?;
	if consumed != ret_str.len() {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("Trailing input: '{}' while parsing '{}'", &ret_str[consumed..], sig),
		));
	}

	let parameters: Vec<Parameter> = args
		.into_iter()
		.enumerate()
		.map(|(i, ty)| Parameter {
			name: Ident::new(&format!("arg{i}"), Span::call_site()),
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

	if matches!(field_type, SigType::Alias(ref name) if name == "void") {
		return Err(syn::Error::new(
			Span::call_site(),
			format!("void cannot be used as a field type in signature: '{sig}'"),
		));
	}

	Ok(Signature::Field(FieldSignature { field_type }))
}

/// Parse a single JNI type descriptor from a string
/// Returns the `JavaType` and the number of bytes consumed
pub(super) fn parse_raw_jni_type(s: &str) -> Result<(SigType, usize)> {
	let bytes: &[u8] = s.as_bytes();
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
			let (elem_type, elem_consumed) = parse_raw_jni_type(&s[1..])?;

			if matches!(elem_type, SigType::Alias(ref name) if name == "void") {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("void cannot be used as an array element type in signature: '{s}'"),
				));
			}

			Ok((SigType::Array(Box::new(elem_type), 1), 1 + elem_consumed))
		}
		'L' => {
			let end: usize = s
				.find(';')
				.ok_or_else(|| syn::Error::new(Span::call_site(), format!("Object type missing closing ';': '{s}'")))?;

			let class_name: &str = &s[1..end];

			if class_name.is_empty() {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Empty class name in object type: '{s}'"),
				));
			}
			if class_name.starts_with('/') {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name cannot start with '/': '{s}'"),
				));
			}
			if class_name.ends_with('/') {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name cannot end with '/': '{s}'"),
				));
			}

			if class_name.contains("//") {
				return Err(syn::Error::new(
					Span::call_site(),
					format!("Class name contains empty segment: '{s}'"),
				));
			}

			let parts: Vec<&str> = class_name.split('/').collect();

			let class: String = parts
				.last()
				.expect("splitting a class name yields at least one component")
				.to_string();

			let package: Vec<String> = parts[..parts.len() - 1]
				.iter()
				.map(std::string::ToString::to_string)
				.collect();

			let java_class = JavaClassName { package, class };

			Ok((SigType::Object(java_class), end + 1))
		}
		c => Err(syn::Error::new(
			Span::call_site(),
			format!("Invalid type descriptor: '{c}'"),
		)),
	}
}
