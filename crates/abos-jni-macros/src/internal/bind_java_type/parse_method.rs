use super::{
	AbiCheck, Constructor, Ident, LitBool, LitStr, Method, MethodKind, MethodSignature, NativeMethod,
	NativeMethodExport, ParseStream, ParsedMethod, Result, SigType, Token, TypeMappings, VisibilitySpec, abi_check,
	braced, catch_unwind, error_policy, export, name, non_null, parenthesized, parse_method_sig,
	parse_parameter_with_index, parse_type, raw, sig, snake_case_to_lower_camel_case,
};

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
				let path: syn::Path = content.parse::<syn::Path>()?;
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

/// Parse a name = "value" or name = `rust_name` pair
pub(super) fn parse_name_spec(input: ParseStream) -> Result<String> {
	input.parse::<name>()?;
	input.parse::<Token![=]>()?;

	if input.peek(LitStr) {
		let lit: LitStr = input.parse::<LitStr>()?;
		// Java lookup names need not be valid Rust identifiers.
		Ok(lit.value())
	} else {
		let rust_name: Ident = input.parse::<Ident>()?;
		Ok(rust_name.to_string())
	}
}

#[derive(Default)]
struct MethodOptions {
	java_name: Option<String>,
	signature: Option<MethodSignature>,
	error_policy: Option<syn::Path>,
	export: Option<NativeMethodExport>,
	native_fn: Option<syn::Path>,
	abi_check: Option<AbiCheck>,
	catch_unwind: Option<bool>,
	is_static: bool,
	is_raw: bool,
	extern_qualifier: Option<Token![extern]>,
	non_null: bool,
}

/// Parse shorthand or property-block syntax and validate the selected method kind.
pub(super) fn parse_method(input: ParseStream, type_mappings: &TypeMappings, kind: MethodKind) -> Result<ParsedMethod> {
	let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;
	let visibility: Option<VisibilitySpec> = parse_visibility(input)?;
	let mut options: MethodOptions = MethodOptions::default();
	options.parse_qualifiers(input)?;
	if !input.peek(Token![fn]) {
		return Err(syn::Error::new(
			input.span(),
			"Expected 'fn' keyword before method name",
		));
	}
	input.parse::<Token![fn]>()?;
	let rust_name: Ident = input.parse()?;
	if input.peek(syn::token::Paren) {
		options.signature = Some(parse_shorthand(input, type_mappings)?);
	} else if input.peek(syn::token::Brace) || input.peek(Token![=]) {
		options.parse_body(input, type_mappings)?;
	} else {
		return Err(syn::Error::new(
			rust_name.span(),
			"Method must be followed by parentheses (shorthand syntax) or braces (block syntax)",
		));
	}
	let java_name_overridden: bool = options.java_name.is_some();
	let parsed: ParsedMethod = options.finish(rust_name, attrs, visibility)?;
	validate_method(&parsed, kind, java_name_overridden, type_mappings)?;
	Ok(parsed)
}

impl MethodOptions {
	fn parse_qualifiers(&mut self, input: ParseStream) -> Result<()> {
		loop {
			if input.peek(Token![static]) {
				input.parse::<Token![static]>()?;
				self.is_static = true;
			} else if input.peek(Token![extern]) {
				self.extern_qualifier = Some(input.parse::<Token![extern]>()?);
			} else if input.peek(raw) {
				input.parse::<raw>()?;
				self.is_raw = true;
			} else if input.peek(non_null) {
				input.parse::<non_null>()?;
				self.non_null = true;
			} else {
				return Ok(());
			}
		}
	}

	fn parse_body(&mut self, input: ParseStream, mappings: &TypeMappings) -> Result<()> {
		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
		}
		let body;
		braced!(body in input);
		while !body.is_empty() {
			self.parse_property(&body, mappings)?;
			if !body.is_empty() {
				body.parse::<Token![,]>()?;
			}
		}
		Ok(())
	}

	fn parse_property(&mut self, input: ParseStream, mappings: &TypeMappings) -> Result<()> {
		let lookahead: syn::parse::Lookahead1<'_> = input.lookahead1();
		if lookahead.peek(name) {
			self.java_name = Some(parse_name_spec(input)?);
		} else if lookahead.peek(sig) {
			self.signature = Some(parse_method_sig(input, mappings)?);
		} else if lookahead.peek(Token![fn]) {
			input.parse::<Token![fn]>()?;
			input.parse::<Token![=]>()?;
			self.native_fn = Some(input.parse()?);
		} else if lookahead.peek(Token![static]) {
			input.parse::<Token![static]>()?;
			input.parse::<Token![=]>()?;
			self.is_static = input.parse::<LitBool>()?.value();
		} else if lookahead.peek(raw) {
			input.parse::<raw>()?;
			input.parse::<Token![=]>()?;
			self.is_raw = input.parse::<LitBool>()?.value();
		} else if lookahead.peek(error_policy) {
			input.parse::<error_policy>()?;
			input.parse::<Token![=]>()?;
			self.error_policy = Some(input.parse()?);
		} else if lookahead.peek(export) {
			input.parse::<export>()?;
			input.parse::<Token![=]>()?;
			self.export = Some(parse_export(input)?);
		} else if lookahead.peek(abi_check) {
			input.parse::<abi_check>()?;
			input.parse::<Token![=]>()?;
			self.abi_check = Some(input.parse()?);
		} else if lookahead.peek(catch_unwind) {
			input.parse::<catch_unwind>()?;
			input.parse::<Token![=]>()?;
			self.catch_unwind = Some(input.parse::<LitBool>()?.value());
		} else if lookahead.peek(non_null) {
			input.parse::<non_null>()?;
			input.parse::<Token![=]>()?;
			self.non_null = input.parse::<LitBool>()?.value();
		} else {
			return Err(lookahead.error());
		}
		Ok(())
	}

	fn finish(
		mut self,
		rust_name: Ident,
		attrs: Vec<syn::Attribute>,
		visibility: Option<VisibilitySpec>,
	) -> Result<ParsedMethod> {
		if self.extern_qualifier.is_some() {
			if self.export.is_some() {
				return Err(syn::Error::new(
					rust_name.span(),
					"Cannot specify both 'extern' qualifier and 'export' property",
				));
			}
			self.export = Some(NativeMethodExport::WithAutoMangle);
		}
		let signature: MethodSignature = self
			.signature
			.ok_or_else(|| syn::Error::new(rust_name.span(), "Method must have a signature"))?;
		let java_name: String = self
			.java_name
			.unwrap_or_else(|| snake_case_to_lower_camel_case(&rust_name.to_string()));
		Ok(ParsedMethod {
			rust_name,
			java_name,
			signature,
			visibility,
			attrs,
			error_policy: self.error_policy,
			export: self.export,
			native_fn: self.native_fn,
			is_raw: self.is_raw,
			is_static: self.is_static,
			abi_check: self.abi_check,
			catch_unwind: self.catch_unwind,
			non_null: self.non_null,
		})
	}
}

fn parse_export(input: ParseStream) -> Result<NativeMethodExport> {
	if input.peek(LitStr) {
		return Ok(NativeMethodExport::WithName(input.parse::<LitStr>()?.value()));
	}
	if input.peek(LitBool) {
		return Ok(if input.parse::<LitBool>()?.value() {
			NativeMethodExport::WithAutoMangle
		} else {
			NativeMethodExport::No
		});
	}
	Err(syn::Error::new(
		input.span(),
		"export must be 'true', 'false', or a string literal",
	))
}

fn parse_shorthand(input: ParseStream, mappings: &TypeMappings) -> Result<MethodSignature> {
	let content;
	parenthesized!(content in input);
	let mut parameters: Vec<crate::internal::signature::Parameter> = Vec::new();
	while !content.is_empty() {
		parameters.push(parse_parameter_with_index(&content, parameters.len(), mappings)?);
		if !content.is_empty() {
			content.parse::<Token![,]>()?;
		}
	}
	let return_type: SigType = if input.peek(Token![->]) {
		input.parse::<Token![->]>()?;
		parse_type(input, mappings)?
	} else {
		SigType::Alias("void".to_string())
	};
	Ok(MethodSignature {
		parameters,
		return_type,
	})
}

fn validate_method(
	method: &ParsedMethod,
	kind: MethodKind,
	name_overridden: bool,
	mappings: &TypeMappings,
) -> Result<()> {
	match kind {
		MethodKind::Constructor => validate_constructor(method, name_overridden)?,
		MethodKind::NativeMethod => validate_native_method(method)?,
		MethodKind::Method => {}
	}
	if kind != MethodKind::NativeMethod {
		validate_non_native_method(method)?;
	}
	if method.non_null
		&& (method.signature.return_type.try_as_primitive(mappings).is_some()
			|| matches!(&method.signature.return_type, SigType::Alias(name) if name == "void"))
	{
		return Err(syn::Error::new(
			method.rust_name.span(),
			"Cannot use 'non_null' with methods that return primitive types or void - non_null is only valid for object/reference return types",
		));
	}
	Ok(())
}

fn validate_constructor(method: &ParsedMethod, name_overridden: bool) -> Result<()> {
	let checks: [(bool, &str); 4] = [
		(
			name_overridden,
			"Cannot override 'name' for constructors - the Java constructor name is always '<init>'",
		),
		(
			!matches!(&method.signature.return_type, SigType::Alias(name) if name == "void"),
			"Constructors must return void - either omit '-> type' or use explicit '-> void' or '-> ()'",
		),
		(method.is_static, "Constructors cannot be static"),
		(
			method.non_null,
			"Constructors cannot use 'non_null' - use it on methods that can return null instead",
		),
	];
	validate_options(method, &checks)
}

fn validate_native_method(method: &ParsedMethod) -> Result<()> {
	if !method.is_raw {
		return Ok(());
	}
	let checks: [(bool, &str); 3] = [
		(
			method.error_policy.is_some(),
			"Cannot specify 'error_policy' when using 'raw' - error_policy only applies to safe wrappers that use with_env",
		),
		(
			method.catch_unwind.is_some(),
			"Cannot specify 'catch_unwind' when using 'raw' - catch_unwind only applies to safe wrappers",
		),
		(
			method.non_null,
			"Cannot specify 'non_null' when using 'raw' - non_null only applies to safe wrappers that return Result",
		),
	];
	validate_options(method, &checks)
}

fn validate_non_native_method(method: &ParsedMethod) -> Result<()> {
	let checks: [(bool, &str); 6] = [
		(
			method.error_policy.is_some(),
			"Only native methods can have 'error_policy'",
		),
		(
			method.export.is_some(),
			"Only native methods can have extern qualifier or 'export =' property",
		),
		(method.native_fn.is_some(), "Only native methods can have 'fn'"),
		(method.is_raw, "Only native methods can have 'raw'"),
		(method.abi_check.is_some(), "Only native methods can have 'abi_check'"),
		(
			method.catch_unwind.is_some(),
			"Only native methods can have 'catch_unwind'",
		),
	];
	validate_options(method, &checks)
}

fn validate_options(method: &ParsedMethod, checks: &[(bool, &str)]) -> Result<()> {
	for &(invalid, message) in checks {
		if invalid {
			return Err(syn::Error::new(method.rust_name.span(), message));
		}
	}
	Ok(())
}

/// Parse constructors block
pub(super) fn parse_constructors(input: ParseStream, type_mappings: &TypeMappings) -> Result<Vec<Constructor>> {
	let mut constructors: Vec<Constructor> = Vec::new();

	while !input.is_empty() {
		let parsed: ParsedMethod = parse_method(input, type_mappings, MethodKind::Constructor)?;

		constructors.push(Constructor {
			name: parsed.rust_name,
			signature: parsed.signature,
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
	let mut methods: Vec<Method> = Vec::new();

	while !input.is_empty() {
		let parsed: ParsedMethod = parse_method(input, type_mappings, MethodKind::Method)?;

		methods.push(Method {
			visibility: parsed.visibility.unwrap_or(VisibilitySpec::Public),
			java_name: parsed.java_name,
			rust_name: parsed.rust_name,
			signature: parsed.signature,
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

/// Parse `native_methods` block
pub(super) fn parse_native_methods(
	input: ParseStream,
	type_mappings: &TypeMappings,
) -> Result<(Vec<NativeMethod>, Vec<Method>)> {
	let mut native_methods: Vec<NativeMethod> = Vec::new();
	let mut methods: Vec<Method> = Vec::new();

	while !input.is_empty() {
		let parsed: ParsedMethod = parse_method(input, type_mappings, MethodKind::NativeMethod)?;

		if let Some(visibility) = parsed.visibility {
			methods.push(Method {
				visibility,
				java_name: parsed.java_name.clone(),
				rust_name: parsed.rust_name.clone(),
				signature: parsed.signature.clone(),
				attrs: parsed.attrs.clone(),
				is_static: parsed.is_static,
				non_null: parsed.non_null,
			});
		}
		native_methods.push(NativeMethod {
			java_name: parsed.java_name,
			rust_name: parsed.rust_name,
			signature: parsed.signature,
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
