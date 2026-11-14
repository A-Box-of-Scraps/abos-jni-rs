use super::{
	AbiCheck, BindClassInput, Constructor, Field, Ident, IdentExt, IsInstanceOfEntry, JavaClassName, LitBool, Method,
	NativeMethod, Parse, ParseStream, Result, Span, Token, TokenStream, TypeMappings, VisibilitySpec, abi_check, api,
	braced, constructors, fields, hooks, is_instance_of, java_type, methods, native_methods,
	native_methods_catch_unwind, native_methods_error_policy, native_methods_export, native_trait, parse_constructors,
	parse_fields, parse_methods, parse_native_methods, parse_visibility, priv_type, quote, rust_type, rust_type_vis,
	type_map,
};

enum PropertyName {
	RustType,
	RustTypeVis,
	JavaType,
	Api,
	PrivType,
	NativeTrait,
	IsInstanceOf,
	TypeMap,
	Hooks,
	Constructors,
	Methods,
	Fields,
	NativeMethods,
	NativeMethodsExport,
	NativeMethodsErrorPolicy,
	AbiCheck,
	NativeMethodsCatchUnwind,
	JniCore,
	SysType,
	Jni,
}

impl PropertyName {
	#[expect(
		clippy::cognitive_complexity,
		reason = "A flat keyword dispatch keeps property recognition and error suggestions together."
	)]
	fn parse(input: ParseStream) -> Result<(Self, Span)> {
		// Only public properties contribute to the unknown-property suggestions.
		let lookahead: syn::parse::Lookahead1<'_> = input.lookahead1();
		let name: Ident = input.parse()?;
		let property: Self = if lookahead.peek(rust_type) {
			Self::RustType
		} else if lookahead.peek(rust_type_vis) {
			Self::RustTypeVis
		} else if lookahead.peek(java_type) {
			Self::JavaType
		} else if lookahead.peek(api) {
			Self::Api
		} else if lookahead.peek(priv_type) {
			Self::PrivType
		} else if lookahead.peek(native_trait) {
			Self::NativeTrait
		} else if lookahead.peek(is_instance_of) {
			Self::IsInstanceOf
		} else if lookahead.peek(type_map) {
			Self::TypeMap
		} else if lookahead.peek(hooks) {
			Self::Hooks
		} else if lookahead.peek(constructors) {
			Self::Constructors
		} else if lookahead.peek(methods) {
			Self::Methods
		} else if lookahead.peek(fields) {
			Self::Fields
		} else if lookahead.peek(native_methods) {
			Self::NativeMethods
		} else if lookahead.peek(native_methods_export) {
			Self::NativeMethodsExport
		} else if lookahead.peek(native_methods_error_policy) {
			Self::NativeMethodsErrorPolicy
		} else if lookahead.peek(abi_check) {
			Self::AbiCheck
		} else if lookahead.peek(native_methods_catch_unwind) {
			Self::NativeMethodsCatchUnwind
		} else if name == "__jni_core" {
			Self::JniCore
		} else if name == "__sys_type" {
			Self::SysType
		} else if name == "jni" {
			Self::Jni
		} else {
			return Err(lookahead.error());
		};
		Ok((property, name.span()))
	}
}

struct ClassParser {
	type_name: Option<Ident>,
	type_visibility: Option<VisibilitySpec>,
	type_attrs: Vec<syn::Attribute>,
	java_class: Option<JavaClassName>,
	api_name: Option<Ident>,
	priv_type: Option<Ident>,
	native_trait_name: Option<Ident>,
	is_instance_of: Vec<IsInstanceOfEntry>,
	type_mappings: TypeMappings,
	constructors: Vec<Constructor>,
	methods: Vec<Method>,
	fields: Vec<Field>,
	native_methods: Vec<NativeMethod>,
	load_class_closure: Option<TokenStream>,
	init_priv_closure: Option<TokenStream>,
	native_methods_export: bool,
	native_methods_catch_unwind: bool,
	abi_check: AbiCheck,
	native_methods_error_policy: Option<syn::Path>,
	jni_core: bool,
	jni_crate: syn::Path,
	sys_type: Option<Ident>,
}

impl ClassParser {
	fn new(jni_crate: syn::Path) -> Self {
		Self {
			type_name: None,
			type_visibility: None,
			type_attrs: Vec::new(),
			java_class: None,
			api_name: None,
			priv_type: None,
			native_trait_name: None,
			is_instance_of: Vec::new(),
			type_mappings: TypeMappings::new(&jni_crate),
			constructors: Vec::new(),
			methods: Vec::new(),
			fields: Vec::new(),
			native_methods: Vec::new(),
			load_class_closure: None,
			init_priv_closure: None,
			native_methods_export: true,
			native_methods_catch_unwind: true,
			abi_check: AbiCheck::default(),
			native_methods_error_policy: None,
			jni_core: false,
			jni_crate,
			sys_type: None,
		}
	}

	fn finish(self, span: proc_macro2::Span) -> Result<BindClassInput> {
		Ok(BindClassInput {
			type_name: self
				.type_name
				.ok_or_else(|| syn::Error::new(span, "Missing required property: rust_type = RustTypeName"))?,
			type_visibility: self.type_visibility,
			type_attrs: self.type_attrs,
			java_class: self
				.java_class
				.ok_or_else(|| syn::Error::new(span, "Missing required property: java_type = java.class.Name"))?,
			api_name: self.api_name,
			priv_type: self.priv_type,
			native_trait_name: self.native_trait_name,
			is_instance_of: self.is_instance_of,
			type_mappings: self.type_mappings,
			constructors: self.constructors,
			methods: self.methods,
			fields: self.fields,
			native_methods: self.native_methods,
			load_class_closure: self.load_class_closure,
			init_priv_closure: self.init_priv_closure,
			native_methods_export: self.native_methods_export,
			native_methods_catch_unwind: self.native_methods_catch_unwind,
			abi_check: self.abi_check,
			native_methods_error_policy: self.native_methods_error_policy,
			jni_core: self.jni_core,
			jni_crate: self.jni_crate,
			sys_type: self.sys_type,
		})
	}
}

impl Parse for BindClassInput {
	fn parse(input: ParseStream) -> Result<Self> {
		let jni: syn::Path = crate::internal::utils::parse_jni_crate_override(input)?;
		let mut parser = ClassParser::new(jni);
		while !input.is_empty() {
			let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;
			let fork: syn::parse::ParseBuffer<'_> = input.fork();
			// Check the shorthand separator before peeking for '=' to avoid splitting '=>'.
			let is_property: bool = fork
				.call(Ident::parse_any)
				.is_ok_and(|_| !fork.peek(Token![=>]) && (fork.peek(Token![=]) || fork.peek(syn::token::Brace)));
			if is_property {
				parser.parse_property(input, attrs)?;
			} else {
				parser.parse_shorthand(input, attrs)?;
			}
			parser.register_self_mapping()?;
			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}
		parser.finish(input.span())
	}
}

impl ClassParser {
	fn parse_shorthand(&mut self, input: ParseStream, attrs: Vec<syn::Attribute>) -> Result<()> {
		self.type_visibility = parse_visibility(input)?;
		let rust_type: Ident = input.parse()?;
		input.parse::<Token![=>]>()?;
		let java_type: JavaClassName = input.parse()?;
		if self.type_name.is_some() || self.java_class.is_some() {
			return Err(syn::Error::new(
				input.span(),
				"The Rust type and Java class can only be specified once",
			));
		}
		self.type_name = Some(rust_type);
		self.type_attrs = attrs;
		self.java_class = Some(java_type);
		Ok(())
	}

	fn parse_property(&mut self, input: ParseStream, attrs: Vec<syn::Attribute>) -> Result<()> {
		let (property, property_span) = PropertyName::parse(input)?;
		match property {
			PropertyName::RustType => self.parse_rust_type(input, property_span, attrs)?,
			PropertyName::RustTypeVis => self.parse_type_visibility(input, property_span)?,
			PropertyName::JavaType => {
				if self.java_class.is_some() {
					return Err(syn::Error::new(property_span, "Java class name already specified"));
				}
				self.java_class = Some(parse_value(input)?);
			}
			PropertyName::Api => self.api_name = Some(parse_value(input)?),
			PropertyName::PrivType => self.priv_type = Some(parse_value(input)?),
			PropertyName::NativeTrait => self.native_trait_name = Some(parse_value(input)?),
			PropertyName::IsInstanceOf => self.is_instance_of.extend(parse_block(input, parse_super_types)?),
			PropertyName::TypeMap => self.type_mappings.parse_mappings(input)?,
			PropertyName::Hooks => parse_block(input, |body| self.parse_hooks(body))?,
			PropertyName::Constructors => {
				self.constructors = parse_block(input, |body| parse_constructors(body, &self.type_mappings))?;
			}
			PropertyName::Methods => self
				.methods
				.extend(parse_block(input, |body| parse_methods(body, &self.type_mappings))?),
			PropertyName::Fields => self
				.fields
				.extend(parse_block(input, |body| parse_fields(body, &self.type_mappings))?),
			PropertyName::NativeMethods => {
				let (native, methods) = parse_block(input, |body| parse_native_methods(body, &self.type_mappings))?;
				self.native_methods.extend(native);
				self.methods.extend(methods);
			}
			PropertyName::NativeMethodsExport => self.native_methods_export = parse_value::<LitBool>(input)?.value(),
			PropertyName::NativeMethodsErrorPolicy => self.native_methods_error_policy = Some(parse_value(input)?),
			PropertyName::AbiCheck => self.abi_check = parse_value(input)?,
			PropertyName::NativeMethodsCatchUnwind => {
				self.native_methods_catch_unwind = parse_value::<LitBool>(input)?.value();
			}
			PropertyName::JniCore => self.jni_core = parse_value::<LitBool>(input)?.value(),
			PropertyName::SysType => self.sys_type = Some(parse_value(input)?),
			PropertyName::Jni => {
				return Err(syn::Error::new(
					property_span,
					"jni property must be the first property if specified",
				));
			}
		}
		Ok(())
	}

	fn parse_rust_type(&mut self, input: ParseStream, property_span: Span, attrs: Vec<syn::Attribute>) -> Result<()> {
		if self.type_name.is_some() {
			return Err(syn::Error::new(property_span, "Rust type name already specified"));
		}
		input.parse::<Token![=]>()?;
		if let Some(vis) = parse_visibility(input)? {
			self.type_visibility = Some(vis);
		}
		self.type_name = Some(input.parse()?);
		self.type_attrs = attrs;
		Ok(())
	}

	fn parse_type_visibility(&mut self, input: ParseStream, property_span: Span) -> Result<()> {
		if self.type_visibility.is_some() {
			return Err(syn::Error::new(property_span, "Type visibility already specified"));
		}
		input.parse::<Token![=]>()?;
		self.type_visibility = Some(parse_visibility(input)?.ok_or_else(|| {
			syn::Error::new(property_span, "Expected visibility specifier after 'type_visibility ='")
		})?);
		Ok(())
	}

	fn parse_hooks(&mut self, input: ParseStream) -> Result<()> {
		while !input.is_empty() {
			let key: Ident = input.parse()?;
			let expression: syn::Expr = parse_value(input)?;
			let tokens: TokenStream = quote! { #expression };
			match key.to_string().as_str() {
				"load_class" => self.load_class_closure = Some(tokens),
				"init_priv" => self.init_priv_closure = Some(tokens),
				_ => return Err(syn::Error::new(key.span(), format!("Unknown hooks property: {key}"))),
			}
			if !input.is_empty() {
				input.parse::<Token![,]>()?;
			}
		}
		Ok(())
	}

	fn register_self_mapping(&mut self) -> Result<()> {
		let (Some(type_name), Some(java_class)) = (&self.type_name, &self.java_class) else {
			return Ok(());
		};
		let path: String = type_name.to_string();
		// Explicit mappings and builtins take precedence over the inferred Self mapping.
		if self.type_mappings.is_core_java_type(java_class) || self.type_mappings.map_alias(&path).is_some() {
			return Ok(());
		}
		self.type_mappings
			.insert_ref_type(&path, &path, java_class.clone(), false)
			.map_err(|err| syn::Error::new(type_name.span(), format!("Failed to register Self type mapping: {err}")))
	}
}

fn parse_value<T: Parse>(input: ParseStream) -> Result<T> {
	input.parse::<Token![=]>()?;
	input.parse()
}

fn parse_block<T>(input: ParseStream, parse: impl FnOnce(ParseStream) -> Result<T>) -> Result<T> {
	if input.peek(Token![=]) {
		input.parse::<Token![=]>()?;
	}
	let body;
	braced!(body in input);
	parse(&body)
}

fn parse_super_types(input: ParseStream) -> Result<Vec<IsInstanceOfEntry>> {
	let mut entries: Vec<IsInstanceOfEntry> = Vec::new();
	while !input.is_empty() {
		let has_stem: bool =
			input.peek(Ident) && !input.peek2(Token![::]) && (input.peek2(Token![=]) || input.peek2(Token![:]));
		let stem: Option<String> = if has_stem {
			let ident: Ident = input.parse()?;
			if input.peek(Token![=]) {
				input.parse::<Token![=]>()?;
			} else {
				input.parse::<Token![:]>()?;
			}
			Some(ident.to_string())
		} else {
			None
		};
		let path: syn::Path = input.parse()?;
		let type_alias: String = quote!(#path).to_string().replace(' ', "");
		if type_alias == "JObject" || type_alias == "abos_jni::objects::JObject" {
			return Err(syn::Error::new_spanned(
				&path,
				"JObject should not be explicitly specified in is_instance_of - all types are already instances of JObject",
			));
		}
		entries.push(IsInstanceOfEntry { type_alias, stem });
		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}
	Ok(entries)
}
