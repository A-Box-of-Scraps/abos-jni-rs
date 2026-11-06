use super::{
	Field, FieldSignature, Ident, LitBool, ParseStream, Result, Token, TypeMappings, VisibilitySpec, braced,
	format_ident, get, name, non_null, parse_field_sig, parse_name_spec, parse_type, parse_visibility, set, sig,
	snake_case_to_lower_camel_case,
};

/// Parse fields or `static_fields` block
pub(super) fn parse_field(input: ParseStream, type_mappings: &TypeMappings) -> Result<Field> {
	// Parse attributes first (for the field itself)
	let field_attrs = input.call(syn::Attribute::parse_outer)?;

	// Parse optional visibility specifier before the field name
	let field_visibility = parse_visibility(input)?;
	// Use field_visibility for both getter and setter, defaulting to Public
	let mut getter_visibility = field_visibility.clone().unwrap_or(VisibilitySpec::Public);
	let mut setter_visibility = field_visibility.unwrap_or(VisibilitySpec::Public);

	// Parse qualifiers that can appear after visibility and before the field name
	// For shorthand: [vis] [static] [non_null] name: Type
	// For block: [vis] [static] name { ... }
	let mut is_static = false;
	let mut is_non_null = false;

	// Parse qualifiers in any order (static, non_null)
	loop {
		if input.peek(Token![static]) {
			input.parse::<Token![static]>()?;
			is_static = true;
		} else if input.peek(non_null) {
			input.parse::<non_null>()?;
			is_non_null = true;
		} else {
			break;
		}
	}

	let rust_name = input.parse::<Ident>()?;

	let default_getter_name = rust_name.clone();

	// If the rust name has any leading underscores, keep them as a prefix for the setter.
	// A field named `_my_field` would have a setter named `_set_my_field` instead of `set__my_field`

	let rust_name_str = rust_name.to_string();
	let n_underscores = rust_name_str.chars().take_while(|c| *c == '_').count();

	let default_setter_name = format_ident!("{}set_{}", "_".repeat(n_underscores), &rust_name_str[n_underscores..]);

	// Automatically convert rust_name from snake_case to lowerCamelCase
	let mut java_name = snake_case_to_lower_camel_case(&rust_name.to_string());

	// Check if this is shorthand syntax (colon) or block syntax (braces)
	if input.peek(Token![:]) {
		// Shorthand syntax: field_name: Type
		input.parse::<Token![:]>()?;
		let field_type = parse_type(input, type_mappings)?;

		let field_signature = FieldSignature { field_type };

		// Validate non_null usage
		if is_non_null {
			let is_primitive = field_signature.field_type.try_as_primitive(type_mappings).is_some();
			if is_primitive {
				return Err(syn::Error::new(
					rust_name.span(),
					"Cannot use 'non_null' with fields of primitive types - non_null is only valid for object/reference field types",
				));
			}
		}

		Ok(Field {
			java_name,
			rust_name,
			getter_name: Some(default_getter_name),
			setter_name: Some(default_setter_name),
			getter_visibility,
			setter_visibility,
			field_signature,
			attrs: field_attrs,
			getter_attrs: Vec::new(),
			setter_attrs: Vec::new(),
			is_static,
			non_null: is_non_null,
		})
	} else if input.peek(syn::token::Brace) || input.peek(Token![=]) {
		// Block syntax: field_name [=] { name = "javaName", sig = Type, ... }

		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
		}

		let body_content;
		braced!(body_content in input);

		let mut field_signature = None;
		let mut getter_attrs = vec![];
		let mut setter_attrs = vec![];
		let mut getter_override_name = None;
		let mut setter_override_name = None;

		while !body_content.is_empty() {
			let prop_attrs = body_content.call(syn::Attribute::parse_outer)?;

			// Try to parse visibility first (handles pub, pub(...), and priv)
			let vis = parse_visibility(&body_content)?;

			// After parsing visibility (if any), check what follows
			if body_content.peek(get) {
				getter_attrs.extend(prop_attrs);
				if let Some(vis) = vis {
					getter_visibility = vis;
				}
				body_content.parse::<get>()?;
				body_content.parse::<Token![=]>()?;
				getter_override_name = Some(body_content.parse::<Ident>()?);
			} else if body_content.peek(set) {
				setter_attrs.extend(prop_attrs);
				if let Some(vis) = vis {
					setter_visibility = vis;
				}
				body_content.parse::<set>()?;
				body_content.parse::<Token![=]>()?;
				setter_override_name = Some(body_content.parse::<Ident>()?);
			} else if vis.is_some() {
				// We parsed a visibility but it's not followed by get/set
				return Err(syn::Error::new(
					body_content.span(),
					"Visibility specifier must be followed by 'get' or 'set'",
				));
			} else if !prop_attrs.is_empty() {
				// We have buffered attributes but didn't find get/set
				return Err(syn::Error::new(
					body_content.span(),
					"Field property attributes must be followed by 'get' or 'set'",
				));
			} else {
				// No visibility, no attributes, not get/set - check for other properties
				let lookahead = body_content.lookahead1();

				if lookahead.peek(name) {
					let (jname, _) = parse_name_spec(&body_content)?;
					java_name = jname;
				} else if lookahead.peek(sig) {
					field_signature = Some(parse_field_sig(&body_content, type_mappings)?);
				} else if lookahead.peek(Token![static]) {
					body_content.parse::<Token![static]>()?;
					body_content.parse::<Token![=]>()?;
					let lit_bool = body_content.parse::<LitBool>()?;
					is_static = lit_bool.value();
				} else if lookahead.peek(non_null) {
					body_content.parse::<non_null>()?;
					body_content.parse::<Token![=]>()?;
					let lit_bool = body_content.parse::<LitBool>()?;
					is_non_null = lit_bool.value();
				} else {
					return Err(lookahead.error());
				}
			}

			if !body_content.is_empty() {
				body_content.parse::<Token![,]>()?;
			}
		}

		let field_signature =
			field_signature.ok_or_else(|| syn::Error::new(rust_name.span(), "Field must have a 'sig' property"))?;

		// Explicitly overriding the getter/setter name but not the other should result in
		// us only generating the specified one.
		let (getter_name, setter_name) = if getter_override_name.is_some() || setter_override_name.is_some() {
			(getter_override_name, setter_override_name)
		} else {
			(Some(default_getter_name), Some(default_setter_name))
		};

		// Validate non_null usage
		if is_non_null {
			let is_primitive = field_signature.field_type.try_as_primitive(type_mappings).is_some();
			if is_primitive {
				return Err(syn::Error::new(
					rust_name.span(),
					"Cannot use 'non_null' with fields of primitive types - non_null is only valid for object/reference field types",
				));
			}
		}

		Ok(Field {
			java_name,
			rust_name,
			getter_name,
			setter_name,
			getter_visibility,
			setter_visibility,
			field_signature,
			attrs: field_attrs,
			getter_attrs,
			setter_attrs,
			is_static,
			non_null: is_non_null,
		})
	} else {
		Err(syn::Error::new(
			rust_name.span(),
			"Field must be followed by colon (shorthand syntax) or braces (block syntax)",
		))
	}
}

/// Parse methods or `static_methods` block
pub(super) fn parse_fields(input: ParseStream, type_mappings: &TypeMappings) -> Result<Vec<Field>> {
	let mut fields = Vec::new();

	while !input.is_empty() {
		let field = parse_field(input, type_mappings)?;

		fields.push(field);

		if !input.is_empty() {
			input.parse::<Token![,]>()?;
		}
	}

	Ok(fields)
}
