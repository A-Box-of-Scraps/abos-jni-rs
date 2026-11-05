use super::*;

/// Parse an optional visibility specifier
/// Supports: pub, pub(self), pub(crate), pub(super), pub(in path), or priv (which maps to pub(self))
/// Returns None if no visibility specifier is present (defaults to Public for methods / constructors)
pub(super) fn parse_visibility(input: ParseStream) -> Result<Option<VisibilitySpec>> {
	if input.peek(Token![pub]) {
		input.parse::<Token![pub]>()?;

		// Check for pub(...)
		if input.peek(syn::token::Paren) {
			let content;
			parenthesized!(content in input);

			if content.peek(Token![self]) {
				content.parse::<Token![self]>()?;
				Ok(Some(VisibilitySpec::PubSelf))
			} else if content.peek(Token![crate]) {
				content.parse::<Token![crate]>()?;
				Ok(Some(VisibilitySpec::PubCrate))
			} else if content.peek(Token![super]) {
				content.parse::<Token![super]>()?;
				Ok(Some(VisibilitySpec::PubSuper))
			} else if content.peek(Token![in]) {
				content.parse::<Token![in]>()?;
				let path = content.parse::<syn::Path>()?;
				Ok(Some(VisibilitySpec::PubInPath(path)))
			} else {
				Err(syn::Error::new(
					content.span(),
					"expected `self`, `crate`, `super`, or `in <path>` after `pub(`",
				))
			}
		} else {
			Ok(Some(VisibilitySpec::Public))
		}
	} else if input.peek(Token![priv]) {
		input.parse::<Token![priv]>()?;
		Ok(Some(VisibilitySpec::PubSelf))
	} else {
		Ok(None)
	}
}

/// Parse a name = "value" or name = rust_name pair
pub(super) fn parse_name_spec(input: ParseStream) -> Result<(String, Ident)> {
	input.parse::<name>()?;
	input.parse::<Token![=]>()?;

	if input.peek(LitStr) {
		let lit = input.parse::<LitStr>()?;
		let java_name = lit.value();
		let rust_name = format_ident!("{}", java_name);
		Ok((java_name, rust_name))
	} else {
		let rust_name = input.parse::<Ident>()?;
		let java_name = rust_name.to_string();
		Ok((java_name, rust_name))
	}
}

/// Unified parser for method blocks (constructors, methods, and native methods)
/// Parses both shorthand syntax: `[vis] [static] [raw] [extern] fn name(params) -> ret`
/// and block syntax: `[vis] [static] [raw] [extern] fn name [=] { ... }`
/// Ensures signature is always present and java_name is derived from rust_name if not explicit
/// Validates based on the method kind
pub(super) fn parse_method(input: ParseStream, type_mappings: &TypeMappings, kind: MethodKind) -> Result<ParsedMethod> {
	// Parse attributes first
	let attrs = input.call(syn::Attribute::parse_outer)?;

	// Parse optional visibility specifier before the method name
	let visibility = parse_visibility(input)?;

	// Parse qualifiers that can appear after visibility and before the fn keyword
	// For shorthand: [vis] [static] [raw] [extern] [non_null] fn name(params) -> ret
	// For block: [vis] [static] [extern] fn name { ... }
	let mut is_static = false;
	let mut is_raw = false;
	let mut is_extern = false;
	let mut is_non_null = false;

	// Parse qualifiers in any order (static, raw, extern, non_null)
	loop {
		if input.peek(Token![static]) {
			input.parse::<Token![static]>()?;
			is_static = true;
		} else if input.peek(Token![extern]) {
			input.parse::<Token![extern]>()?;
			is_extern = true;
		} else if input.peek(raw) {
			input.parse::<raw>()?;
			is_raw = true;
		} else if input.peek(non_null) {
			input.parse::<non_null>()?;
			is_non_null = true;
		} else {
			break;
		}
	}

	// Require 'fn' keyword for both shorthand and block syntax
	if !input.peek(Token![fn]) {
		return Err(syn::Error::new(
			input.span(),
			"Expected 'fn' keyword before method name",
		));
	}
	input.parse::<Token![fn]>()?;

	let rust_name = input.parse::<Ident>()?;

	let mut java_name = None;
	let mut method_signature = None;
	let mut error_policy = None;
	let mut export = None;
	let mut native_fn = None;
	let mut abi_check = None;
	let mut catch_unwind = None;

	// Check if this is shorthand syntax (parentheses) or block syntax (braces)
	if input.peek(syn::token::Paren) {
		// Shorthand syntax: [vis] [qual..] fn name(params) [-> return_type]
		let sig_content;
		parenthesized!(sig_content in input);

		let mut parameters = Vec::new();
		let mut param_index = 0;
		while !sig_content.is_empty() {
			let param = parse_parameter_with_index(&sig_content, param_index, type_mappings)?;
			parameters.push(param);
			param_index += 1;

			if !sig_content.is_empty() {
				sig_content.parse::<Token![,]>()?;
			}
		}

		// Check if there's a return type (-> Type)
		// Constructors don't have return types, methods do
		let return_type = if input.peek(Token![->]) {
			input.parse::<Token![->]>()?;
			parse_type(input, type_mappings)?
		} else {
			// No return type means this is constructor shorthand - use void
			SigType::Alias("void".to_string())
		};

		method_signature = Some(MethodSignature {
			parameters,
			return_type,
		});
	} else if input.peek(syn::token::Brace) || input.peek(Token![=]) {
		// Block syntax: [vis] [qual..] fn name [=] { name = "javaName", sig = (params) -> ret, ... }

		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
		}

		let body_content;
		braced!(body_content in input);

		while !body_content.is_empty() {
			let lookahead = body_content.lookahead1();

			if lookahead.peek(name) {
				let (jname, _) = parse_name_spec(&body_content)?;
				java_name = Some(jname);
			} else if lookahead.peek(sig) {
				method_signature = Some(parse_method_sig(&body_content, type_mappings)?);
			} else if lookahead.peek(Token![fn]) {
				// Parse: fn = path
				body_content.parse::<Token![fn]>()?;
				body_content.parse::<Token![=]>()?;
				let path = body_content.parse::<syn::Path>()?;
				native_fn = Some(path);
			} else if lookahead.peek(Token![static]) {
				body_content.parse::<Token![static]>()?;
				body_content.parse::<Token![=]>()?;
				let lit_bool = body_content.parse::<LitBool>()?;
				is_static = lit_bool.value();
			} else if lookahead.peek(raw) {
				body_content.parse::<raw>()?;
				body_content.parse::<Token![=]>()?;
				let lit_bool = body_content.parse::<LitBool>()?;
				is_raw = lit_bool.value();
			} else if lookahead.peek(self::error_policy) {
				body_content.parse::<error_policy>()?;
				body_content.parse::<Token![=]>()?;
				error_policy = Some(body_content.parse::<syn::Path>()?);
			} else if lookahead.peek(self::export) {
				body_content.parse::<export>()?;
				body_content.parse::<Token![=]>()?;
				if body_content.peek(LitStr) {
					// export = "customName"
					let name = body_content.parse::<LitStr>()?.value();
					export = Some(NativeMethodExport::WithName(name));
				} else if body_content.peek(LitBool) {
					// export = true or export = false
					let lit_bool = body_content.parse::<LitBool>()?;
					export = Some(if lit_bool.value() {
						NativeMethodExport::WithAutoMangle
					} else {
						NativeMethodExport::No
					});
				} else {
					return Err(syn::Error::new(
						body_content.span(),
						"export must be 'true', 'false', or a string literal",
					));
				}
			} else if lookahead.peek(self::abi_check) {
				body_content.parse::<abi_check>()?;
				body_content.parse::<Token![=]>()?;
				abi_check = Some(body_content.parse::<AbiCheck>()?);
			} else if lookahead.peek(self::catch_unwind) {
				body_content.parse::<catch_unwind>()?;
				body_content.parse::<Token![=]>()?;
				let lit_bool = body_content.parse::<LitBool>()?;
				catch_unwind = Some(lit_bool.value());
			} else if lookahead.peek(self::non_null) {
				body_content.parse::<non_null>()?;
				body_content.parse::<Token![=]>()?;
				let lit_bool = body_content.parse::<LitBool>()?;
				is_non_null = lit_bool.value();
			} else {
				return Err(lookahead.error());
			}

			// Require comma between properties, but trailing comma is optional
			if !body_content.is_empty() {
				body_content.parse::<Token![,]>()?;
			}
		}
	} else {
		return Err(syn::Error::new(
			rust_name.span(),
			"Method must be followed by parentheses (shorthand syntax) or braces (block syntax)",
		));
	}

	// If extern was specified before the name, it means export = true
	if is_extern {
		if export.is_some() {
			return Err(syn::Error::new(
				rust_name.span(),
				"Cannot specify both 'extern' qualifier and 'export' property",
			));
		}
		export = Some(NativeMethodExport::WithAutoMangle);
	}

	// Require signature - all methods must have one
	let method_signature =
		method_signature.ok_or_else(|| syn::Error::new(rust_name.span(), "Method must have a signature"))?;

	// Track whether java_name was explicitly overridden
	let java_name_overridden = java_name.is_some();

	// Set java_name: use explicit name if provided, otherwise convert from rust_name
	let java_name = java_name.unwrap_or_else(|| snake_case_to_lower_camel_case(&rust_name.to_string()));

	// Perform kind-specific validation
	match kind {
		MethodKind::Constructor => {
			// Constructors cannot have explicit java name (it's always "<init>")
			if java_name_overridden {
				return Err(syn::Error::new(
					rust_name.span(),
					"Cannot override 'name' for constructors - the Java constructor name is always '<init>'",
				));
			}

			// Constructors must return void
			if !matches!(
				method_signature.return_type,
				SigType::Alias(ref name) if name == "void"
			) {
				return Err(syn::Error::new(
					rust_name.span(),
					"Constructors must return void - either omit '-> type' or use explicit '-> void' or '-> ()'",
				));
			}

			// Constructors cannot be static
			if is_static {
				return Err(syn::Error::new(rust_name.span(), "Constructors cannot be static"));
			}

			// Constructors cannot use non_null
			if is_non_null {
				return Err(syn::Error::new(
					rust_name.span(),
					"Constructors cannot use 'non_null' - use it on methods that can return null instead",
				));
			}
		}
		MethodKind::Method => {
			// See !NativeMethod validation below
		}
		MethodKind::NativeMethod => {
			// Validate that raw methods and error_policy are not both specified
			// error_policy only applies when we wrap with with_env
			if is_raw {
				if error_policy.is_some() {
					return Err(syn::Error::new(
						rust_name.span(),
						"Cannot specify 'error_policy' when using 'raw' - error_policy only applies to safe wrappers that use with_env",
					));
				}
				if catch_unwind.is_some() {
					return Err(syn::Error::new(
						rust_name.span(),
						"Cannot specify 'catch_unwind' when using 'raw' - catch_unwind only applies to safe wrappers",
					));
				}
				if is_non_null {
					return Err(syn::Error::new(
						rust_name.span(),
						"Cannot specify 'non_null' when using 'raw' - non_null only applies to safe wrappers that return Result",
					));
				}
			}
		}
	}

	// Common validation for non-native methods
	if kind != MethodKind::NativeMethod {
		// Only native methods can have error_policy types
		if error_policy.is_some() {
			return Err(syn::Error::new(
				rust_name.span(),
				"Only native methods can have 'error_policy'",
			));
		}

		// Only native methods can have export
		if export.is_some() {
			return Err(syn::Error::new(
				rust_name.span(),
				"Only native methods can have extern qualifier or 'export =' property",
			));
		}

		// Only native methods can have native_fn
		if native_fn.is_some() {
			return Err(syn::Error::new(rust_name.span(), "Only native methods can have 'fn'"));
		}

		// Only native methods can have raw
		if is_raw {
			return Err(syn::Error::new(rust_name.span(), "Only native methods can have 'raw'"));
		}

		if abi_check.is_some() {
			return Err(syn::Error::new(
				rust_name.span(),
				"Only native methods can have 'abi_check'",
			));
		}

		if catch_unwind.is_some() {
			return Err(syn::Error::new(
				rust_name.span(),
				"Only native methods can have 'catch_unwind'",
			));
		}
	}

	// Validate non_null usage
	if is_non_null {
		// Check if the return type is a primitive or void
		let is_primitive_or_void = method_signature.return_type.try_as_primitive(type_mappings).is_some()
			|| matches!(
				method_signature.return_type,
				SigType::Alias(ref name) if name == "void"
			);

		if is_primitive_or_void {
			return Err(syn::Error::new(
				rust_name.span(),
				"Cannot use 'non_null' with methods that return primitive types or void - non_null is only valid for object/reference return types",
			));
		}
	}

	Ok(ParsedMethod {
		rust_name,
		java_name,
		method_signature,
		visibility,
		error_policy,
		export,
		attrs,
		native_fn,
		is_raw,
		is_static,
		abi_check,
		catch_unwind,
		non_null: is_non_null,
	})
}

/// Parse constructors block
pub(super) fn parse_constructors(input: ParseStream, type_mappings: &TypeMappings) -> Result<Vec<Constructor>> {
	let mut constructors = Vec::new();

	while !input.is_empty() {
		let parsed = parse_method(input, type_mappings, MethodKind::Constructor)?;

		constructors.push(Constructor {
			name: parsed.rust_name,
			method_signature: parsed.method_signature,
			attrs: parsed.attrs,
		});

		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
		}
	}

	Ok(constructors)
}

/// Parse methods block
pub(super) fn parse_methods(input: ParseStream, type_mappings: &TypeMappings) -> Result<Vec<Method>> {
	let mut methods = Vec::new();

	while !input.is_empty() {
		let parsed = parse_method(input, type_mappings, MethodKind::Method)?;

		methods.push(Method {
			visibility: parsed.visibility.unwrap_or(VisibilitySpec::Public),
			java_name: parsed.java_name,
			rust_name: parsed.rust_name,
			method_signature: parsed.method_signature,
			attrs: parsed.attrs,
			is_static: parsed.is_static,
			non_null: parsed.non_null,
		});

		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}

	Ok(methods)
}

/// Parse native_methods block
pub(super) fn parse_native_methods(
	input: ParseStream,
	type_mappings: &TypeMappings,
) -> Result<(Vec<NativeMethod>, Vec<Method>)> {
	let mut native_methods = Vec::new();
	let mut methods = Vec::new();

	while !input.is_empty() {
		let parsed = parse_method(input, type_mappings, MethodKind::NativeMethod)?;

		if let Some(visibility) = parsed.visibility {
			methods.push(Method {
				visibility,
				java_name: parsed.java_name.clone(),
				rust_name: parsed.rust_name.clone(),
				method_signature: parsed.method_signature.clone(),
				attrs: parsed.attrs.clone(),
				is_static: parsed.is_static,
				non_null: parsed.non_null,
			});
		}
		native_methods.push(NativeMethod {
			java_name: parsed.java_name,
			rust_name: parsed.rust_name,
			method_signature: parsed.method_signature,
			error_policy: parsed.error_policy,
			export: parsed.export.unwrap_or(NativeMethodExport::Default),
			attrs: parsed.attrs,
			native_fn: parsed.native_fn,
			is_raw: parsed.is_raw,
			is_static: parsed.is_static,
			abi_check: parsed.abi_check,
			catch_unwind: parsed.catch_unwind,
		});

		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}

	Ok((native_methods, methods))
}
