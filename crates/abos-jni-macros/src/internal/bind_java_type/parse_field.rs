use super::{
	Field, FieldSignature, Ident, ParseStream, Result, Token, TypeMappings, VisibilitySpec, format_ident, get, name,
	non_null, parse_field_sig, parse_name_spec, parse_type, parse_visibility, set, sig, snake_case_to_lower_camel_case,
};

struct Accessor {
	name: Option<Ident>,
	visibility: VisibilitySpec,
	attrs: Vec<syn::Attribute>,
}

impl Accessor {
	fn parse(
		&mut self,
		input: ParseStream,
		visibility: Option<VisibilitySpec>,
		attrs: Vec<syn::Attribute>,
	) -> Result<()> {
		self.attrs.extend(attrs);
		if let Some(visibility) = visibility {
			self.visibility = visibility;
		}
		input.parse::<Token![=]>()?;
		self.name = Some(input.parse()?);
		Ok(())
	}
}

struct FieldParser {
	requirement: super::Requirement,
	rust_name: Ident,
	java_name: String,
	signature: Option<FieldSignature>,
	getter: Accessor,
	setter: Accessor,
	attrs: Vec<syn::Attribute>,
	is_static: bool,
	non_null: bool,
}

/// Parse a field in either shorthand or property-block form.
pub(super) fn parse_field(input: ParseStream, type_mappings: &TypeMappings) -> Result<Field> {
	let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;
	let visibility: VisibilitySpec = parse_visibility(input)?.unwrap_or(VisibilitySpec::Public);
	let (is_static, non_null) = parse_qualifiers(input)?;
	let rust_name: Ident = input.parse()?;
	let mut parser = FieldParser {
		requirement: super::Requirement::default(),
		java_name: snake_case_to_lower_camel_case(&rust_name.to_string()),
		rust_name,
		signature: None,
		getter: Accessor {
			name: None,
			visibility: visibility.clone(),
			attrs: Vec::new(),
		},
		setter: Accessor {
			name: None,
			visibility,
			attrs: Vec::new(),
		},
		attrs,
		is_static,
		non_null,
	};
	if input.peek(Token![:]) {
		input.parse::<Token![:]>()?;
		parser.signature = Some(FieldSignature {
			field_type: parse_type(input, type_mappings)?,
		});
	} else if input.peek(syn::token::Brace) || input.peek(Token![=]) {
		super::parse_property_block(input, |property| parser.parse_property(property, type_mappings))?;
	} else {
		return Err(syn::Error::new(
			parser.rust_name.span(),
			"Field must be followed by colon (shorthand syntax) or braces (block syntax)",
		));
	}
	parser.finish(type_mappings)
}

fn parse_qualifiers(input: ParseStream) -> Result<(bool, bool)> {
	let mut is_static = false;
	let mut is_non_null = false;
	loop {
		if input.peek(Token![static]) {
			input.parse::<Token![static]>()?;
			is_static = true;
		} else if input.peek(non_null) {
			input.parse::<non_null>()?;
			is_non_null = true;
		} else {
			return Ok((is_static, is_non_null));
		}
	}
}

impl FieldParser {
	fn parse_property(&mut self, input: ParseStream, mappings: &TypeMappings) -> Result<()> {
		let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;
		let visibility: Option<VisibilitySpec> = parse_visibility(input)?;
		if input.peek(get) {
			input.parse::<get>()?;
			return self.getter.parse(input, visibility, attrs);
		}
		if input.peek(set) {
			input.parse::<set>()?;
			return self.setter.parse(input, visibility, attrs);
		}
		if visibility.is_some() {
			return Err(syn::Error::new(
				input.span(),
				"Visibility specifier must be followed by 'get' or 'set'",
			));
		}
		if !attrs.is_empty() {
			return Err(syn::Error::new(
				input.span(),
				"Field property attributes must be followed by 'get' or 'set'",
			));
		}
		self.parse_value_property(input, mappings)
	}

	fn parse_value_property(&mut self, input: ParseStream, mappings: &TypeMappings) -> Result<()> {
		let lookahead: syn::parse::Lookahead1<'_> = input.lookahead1();
		if lookahead.peek(name) {
			self.java_name = parse_name_spec(input)?;
		} else if lookahead.peek(sig) {
			self.signature = Some(parse_field_sig(input, mappings)?);
		} else if lookahead.peek(Token![static]) {
			self.is_static = super::parse_bool_property::<Token![static]>(input)?;
		} else if lookahead.peek(super::requires) {
			self.requirement = super::Requirement::parse_property(input)?;
		} else if lookahead.peek(non_null) {
			self.non_null = super::parse_bool_property::<non_null>(input)?;
		} else {
			return Err(lookahead.error());
		}
		Ok(())
	}

	fn finish(mut self, mappings: &TypeMappings) -> Result<Field> {
		let signature: FieldSignature = self
			.signature
			.ok_or_else(|| syn::Error::new(self.rust_name.span(), "Field must have a 'sig' property"))?;
		if self.non_null && signature.field_type.try_as_primitive(mappings).is_some() {
			return Err(syn::Error::new(
				self.rust_name.span(),
				"Cannot use 'non_null' with fields of primitive types - non_null is only valid for object/reference field types",
			));
		}
		// An explicit getter or setter suppresses the default for the other accessor.
		if self.getter.name.is_none() && self.setter.name.is_none() {
			self.getter.name = Some(self.rust_name.clone());
			let spelling: String = self.rust_name.to_string();
			let underscores: usize = spelling.chars().take_while(|c| *c == '_').count();
			self.setter.name = Some(format_ident!(
				"{}set_{}",
				"_".repeat(underscores),
				&spelling[underscores..]
			));
		}
		Ok(Field {
			requirement: self.requirement,
			java_name: self.java_name,
			rust_name: self.rust_name,
			getter_name: self.getter.name,
			setter_name: self.setter.name,
			getter_visibility: self.getter.visibility,
			setter_visibility: self.setter.visibility,
			signature,
			attrs: self.attrs,
			getter_attrs: self.getter.attrs,
			setter_attrs: self.setter.attrs,
			is_static: self.is_static,
			non_null: self.non_null,
		})
	}
}

/// Parse a sequence of fields.
pub(super) fn parse_fields(input: ParseStream, type_mappings: &TypeMappings) -> Result<Vec<Field>> {
	let mut fields: Vec<Field> = Vec::new();
	while !input.is_empty() {
		fields.push(parse_field(input, type_mappings)?);
		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}
	Ok(fields)
}
