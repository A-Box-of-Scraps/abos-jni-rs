use super::{
	AbiCheck, Ident, IdentExt, JavaClassName, MethodSignature, NativeMethodExport, NativeMethodStructInput, Parse,
	ParseStream, Result, SigType, Token, TypeMappings, parse_method_sig, parse_parameter_with_index, parse_type, raw,
	sig, snake_case_to_lower_camel_case,
};
use proc_macro2::Span;

struct MethodParser {
	jni_crate: syn::Path,
	type_mappings: TypeMappings,
	rust_type: Option<syn::Path>,
	java_type: Option<JavaClassName>,
	method_name: Option<String>,
	signature: Option<MethodSignature>,
	fn_path: Option<syn::Path>,
	is_static: bool,
	is_raw: bool,
	export: NativeMethodExport,
	abi_check: AbiCheck,
	catch_unwind: Option<bool>,
	error_policy: Option<Ident>,
}

impl MethodParser {
	fn new(jni_crate: syn::Path) -> Self {
		Self {
			type_mappings: TypeMappings::new(&jni_crate),
			jni_crate,
			rust_type: None,
			java_type: None,
			method_name: None,
			signature: None,
			fn_path: None,
			is_static: false,
			is_raw: false,
			export: NativeMethodExport::No,
			abi_check: AbiCheck::default(),
			catch_unwind: None,
			error_policy: None,
		}
	}

	fn parse_property(&mut self, input: ParseStream) -> Result<()> {
		// Signature parsing consumes its own property name and '='.
		if input.peek(sig) {
			self.signature = Some(parse_method_sig(input, &self.type_mappings)?);
			return Ok(());
		}
		let property: Ident = input.call(Ident::parse_any)?;
		if property == "type_map" {
			return self.type_mappings.parse_mappings(input);
		}
		input.parse::<Token![=]>()?;
		match property.to_string().as_str() {
			"rust_type" => self.rust_type = Some(input.parse()?),
			"java_type" => self.java_type = Some(input.parse()?),
			"name" => self.method_name = Some(input.parse::<syn::LitStr>()?.value()),
			"static" => self.is_static = input.parse::<syn::LitBool>()?.value(),
			"raw" => self.is_raw = input.parse::<syn::LitBool>()?.value(),
			"export" => self.export = parse_export(input)?,
			"abi_check" => self.abi_check = input.parse()?,
			"catch_unwind" => self.catch_unwind = Some(input.parse::<syn::LitBool>()?.value()),
			"fn" => self.fn_path = Some(input.parse()?),
			"error_policy" => self.error_policy = Some(input.parse()?),
			_ => {
				return Err(syn::Error::new(
					property.span(),
					format!("Unknown native method property: {property}"),
				));
			}
		}
		Ok(())
	}

	fn parse_qualifiers(&mut self, input: ParseStream) -> Result<()> {
		while input.peek(Token![static]) || input.peek(Token![extern]) || input.peek(raw) {
			let qualifier: Ident = input.call(Ident::parse_any)?;
			match qualifier.to_string().as_str() {
				"static" => self.is_static = true,
				"raw" => self.is_raw = true,
				"extern" if matches!(self.export, NativeMethodExport::No) => {
					self.export = NativeMethodExport::WithAutoMangle;
				}
				// An explicit export name takes precedence over the shorthand qualifier.
				_ => {}
			}
		}
		Ok(())
	}

	fn parse_inline(&mut self, input: ParseStream) -> Result<()> {
		self.parse_qualifiers(input)?;
		if !input.peek(Token![fn]) {
			return Err(syn::Error::new(
				input.span(),
				"Expected 'fn' keyword before RustType::method_name",
			));
		}
		input.parse::<Token![fn]>()?;
		let full_path: syn::Path = input.parse()?;
		let (rust_type, method_name) = split_method_path(&full_path)?;
		self.signature = Some(parse_inline_signature(input, &self.type_mappings)?);
		if self.rust_type.is_none() {
			self.rust_type = rust_type;
		}
		self.method_name
			.get_or_insert_with(|| snake_case_to_lower_camel_case(&method_name));
		// 'fn = path' may appear before or after shorthand and overrides its default path.
		self.fn_path.get_or_insert(full_path);
		Ok(())
	}

	fn validate_modes(&self, span: Span) -> Result<()> {
		if matches!(self.export, NativeMethodExport::WithAutoMangle) && self.java_type.is_none() {
			return Err(syn::Error::new(
				span,
				"java_type = \"...\" is required when 'export = true' or 'extern' qualifier is used in signature",
			));
		}
		if self.is_raw && self.error_policy.is_some() {
			return Err(syn::Error::new(
				span,
				"Cannot specify 'error_policy' when using 'raw = true' or 'raw' in signature - error_policy only applies to wrapped functions",
			));
		}
		if self.is_raw && self.catch_unwind.is_some() {
			return Err(syn::Error::new(
				span,
				"Cannot specify 'catch_unwind' when using 'raw = true' or 'raw' in signature - catch_unwind only applies to wrapped functions",
			));
		}
		Ok(())
	}

	fn finish(mut self, span: Span) -> Result<NativeMethodStructInput> {
		let java_method_name = self.method_name.take().ok_or_else(|| syn::Error::new(span,
			"Missing method name (use 'name = \"javaCamelCaseName\"' or inline signature like 'fn RustType::snake_case_method_name()')"))?;
		let method_signature = self.signature.take().ok_or_else(|| syn::Error::new(span,
			"Missing method signature (use 'sig = (args...) -> ret' or inline signature like 'fn RustType::method_name(args...) -> ret')"))?;
		let fn_path = self.fn_path.take().ok_or_else(|| syn::Error::new(span,
			"Missing 'fn' and can't infer a default function path without an inline signature like 'fn RustType::method_name(args..) -> ret'."))?;
		self.validate_modes(span)?;
		Ok(NativeMethodStructInput {
			is_static: self.is_static,
			jni_crate: self.jni_crate,
			rust_type: self.rust_type,
			java_type: self.java_type,
			java_method_name,
			method_signature,
			fn_path,
			type_mappings: self.type_mappings,
			is_raw_fn: self.is_raw,
			error_policy: self.error_policy,
			export: self.export,
			abi_check: self.abi_check,
			catch_unwind: self.catch_unwind.unwrap_or(true),
		})
	}
}

fn is_property(input: ParseStream) -> bool {
	let fork = input.fork();
	fork.call(Ident::parse_any).is_ok() && (fork.peek(Token![=]) || fork.peek(syn::token::Brace))
}

fn parse_export(input: ParseStream) -> Result<NativeMethodExport> {
	if input.peek(syn::LitStr) {
		return Ok(NativeMethodExport::WithName(input.parse::<syn::LitStr>()?.value()));
	}
	if input.peek(syn::LitBool) {
		return Ok(if input.parse::<syn::LitBool>()?.value() {
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

fn split_method_path(path: &syn::Path) -> Result<(Option<syn::Path>, String)> {
	let mut owner = path.clone();
	let method = owner
		.segments
		.pop()
		.ok_or_else(|| syn::Error::new_spanned(path, "Expected method path"))?;
	let method_name = method.ident.to_string();
	// Popping the last segment leaves the preceding separator in Punctuated.
	owner.segments.pop_punct();
	Ok(((!owner.segments.is_empty()).then_some(owner), method_name))
}

fn parse_inline_signature(input: ParseStream, mappings: &TypeMappings) -> Result<MethodSignature> {
	let args;
	syn::parenthesized!(args in input);
	let mut parameters = Vec::new();
	while !args.is_empty() {
		parameters.push(parse_parameter_with_index(&args, parameters.len(), mappings)?);
		if !args.is_empty() {
			args.parse::<Token![,]>()?;
		}
	}
	let return_type = if input.peek(Token![->]) {
		input.parse::<Token![->]>()?;
		parse_type(input, mappings)?
	} else {
		SigType::Alias("void".to_owned())
	};
	Ok(MethodSignature {
		parameters,
		return_type,
	})
}

impl Parse for NativeMethodStructInput {
	fn parse(input: ParseStream) -> Result<Self> {
		let jni_crate = crate::internal::utils::parse_jni_crate_override(&input)?;
		let mut parser = MethodParser::new(jni_crate);
		while !input.is_empty() {
			if is_property(input) {
				parser.parse_property(input)?;
			} else {
				parser.parse_inline(input)?;
			}
			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}
		parser.finish(input.span())
	}
}
