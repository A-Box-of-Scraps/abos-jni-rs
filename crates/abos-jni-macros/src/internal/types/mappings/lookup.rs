use super::{
	ConcreteType, Ident, JavaClassName, ParseStream, PrimitiveType, Rc, Result, RustType, RustTypeTarget, Span, Token,
	TypeMappings, braced, path_to_string_no_spaces, typealias,
};

impl TypeMappings {
	/// Map a Rust reference type to a Java class, replacing the class's reverse lookup.
	///
	/// Rejects core Java classes and aliases already bound to a different Rust type.
	pub fn insert_ref_type(
		&mut self,
		alias: &str,
		path: &str,
		java_class: JavaClassName,
		is_builtin: bool,
	) -> Result<()> {
		let java_class: Rc<JavaClassName> = if let Some((java_class, _)) = self.java_to_rust.get_key_value(&java_class)
		{
			java_class.clone()
		} else {
			Rc::new(java_class)
		};

		if self.core_java.contains(&java_class) {
			return Err(syn::Error::new(
				Span::call_site(),
				format!(
					"Cannot create type mapping for '{}' -> '{}': '{}' is a core Java type. Core types (java.lang.Object, java.lang.Class, java.lang.Throwable and java.lang.String) are automatically mapped to jni crate types and cannot be remapped.",
					path,
					java_class.to_java_dotted(),
					java_class.to_java_dotted()
				),
			));
		}

		let rust_type: Rc<RustType> = self.rust_reference_type(path, is_builtin);

		if let Some(existing) = self.alias_to_rust.get(alias) {
			if existing != &rust_type {
				let existing_java_mapping: &Rc<JavaClassName> = self
					.rust_to_java
					.get(existing)
					.expect("registered reference types have Java mappings");
				return Err(syn::Error::new(
					Span::call_site(),
					format!(
						"Cannot change existing Rust type mapping for Rust type name '{alias}': already mapped to Java class '{}'/'{}', cannot remap to '{}'/'{}'. Only one mapping per Rust type path is allowed.",
						existing_java_mapping.to_java_dotted(),
						existing.path(),
						java_class.to_java_dotted(),
						path
					),
				));
			}

			return Ok(());
		}

		self.alias_to_rust.insert(alias.to_string(), rust_type.clone());

		self.rust_to_java.insert(rust_type.clone(), java_class.clone());
		self.java_to_rust.insert(java_class, rust_type);

		Ok(())
	}

	pub fn insert_prim_type(
		&mut self,
		alias: &str,
		primitive: PrimitiveType,
		path: &str,
		is_builtin: bool,
	) -> Result<()> {
		let rust_type: Rc<RustType> = self.rust_prim_type(primitive, path.to_string(), is_builtin);

		if let Some(existing) = self.alias_to_rust.get(alias) {
			if existing != &rust_type {
				return Err(syn::Error::new(
					Span::call_site(),
					format!(
						"Cannot change existing Rust type mapping for Rust type name '{alias}': already mapped to a different primitive type. Only one mapping per Rust type name is allowed."
					),
				));
			}
			return Ok(());
		}

		self.alias_to_rust.insert(alias.to_string(), rust_type);

		Ok(())
	}

	pub fn insert_alias(&mut self, from: &str, to: &str) -> Result<()> {
		let rust_type: &Rc<RustType> = self.alias_to_rust.get(to).ok_or_else(|| {
			syn::Error::new(
				Span::call_site(),
				format!("Cannot create alias from '{from}' to '{to}': target type not found"),
			)
		})?;
		if let Some(existing) = self.alias_to_rust.get(from) {
			if existing != rust_type {
				return Err(syn::Error::new(
					Span::call_site(),
					format!(
						"Cannot change mapping for typealias '{}' to '{}': already mapped to a different type ({}). Only one mapping per Rust type name is allowed.",
						from,
						to,
						existing.path()
					),
				));
			}
			return Ok(());
		}
		self.alias_to_rust.insert(from.to_string(), rust_type.clone());
		Ok(())
	}

	/// Parse a braced `type_map` block, optionally preceded by `=`.
	///
	/// Entries are comma-separated, with an optional trailing comma:
	/// - `RustPath => java.lang.Class` maps a reference type.
	/// - `unsafe RustPath => long` maps a custom primitive representation.
	/// - `typealias NewName => ExistingName` aliases an existing mapping.
	///
	/// Array aliases are not supported.
	pub fn parse_mappings(&mut self, input: ParseStream) -> Result<()> {
		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
		}

		let mappings_content;
		braced!(mappings_content in input);

		while !mappings_content.is_empty() {
			self.parse_mapping(&mappings_content)?;
			if !mappings_content.is_empty() {
				mappings_content.parse::<Token![,]>()?;
			}
		}

		Ok(())
	}

	fn parse_mapping(&mut self, input: ParseStream) -> Result<()> {
		if input.peek(typealias) {
			input.parse::<typealias>()?;

			let new_alias: syn::Path = input.parse()?;

			input.parse::<Token![=>]>()?;

			let existing_type: syn::Path = input.parse()?;

			let new_alias_str: String = path_to_string_no_spaces(&new_alias);
			let existing_type_str: String = path_to_string_no_spaces(&existing_type);
			self.insert_alias(&new_alias_str, &existing_type_str)?;
		} else {
			let is_prim_mapping: bool = if input.peek(Token![unsafe]) {
				input.parse::<Token![unsafe]>()?;
				true
			} else {
				false
			};

			let rust_path: syn::Path = input.parse()?;

			let rust_path_str: String = path_to_string_no_spaces(&rust_path);

			input.parse::<Token![=>]>()?;

			if is_prim_mapping {
				self.parse_primitive_mapping(input, &rust_path_str)?;
			} else {
				let java_class_mapping: JavaClassName = input.parse()?;

				self.insert_ref_type(&rust_path_str, &rust_path_str, java_class_mapping, false)?;
			}
		}
		Ok(())
	}

	fn parse_primitive_mapping(&mut self, input: ParseStream, rust_path_str: &str) -> Result<()> {
		let prim_type_ident: Ident = input.parse()?;
		let prim_type_name: String = prim_type_ident.to_string();

		match self.map_alias(&prim_type_name) {
			Some(ConcreteType::Primitive {
				primitive, is_builtin, ..
			}) if is_builtin => {
				self.insert_prim_type(rust_path_str, primitive, rust_path_str, false)?;
			}
			_ => {
				return Err(syn::Error::new(
					prim_type_ident.span(),
					format!(
						"Invalid primitive type name '{prim_type_name}' for unsafe primitive mapping. Must be one of the built-in primitive type names (e.g., 'int', 'long', 'boolean', etc.)"
					),
				));
			}
		}
		Ok(())
	}

	/// Get the Java class name and canonical Rust type for a Rust type alias
	pub fn map_alias(&self, alias: &str) -> Option<ConcreteType> {
		self.alias_to_rust.get(alias).map(|rust_type| match &rust_type.target {
			RustTypeTarget::Primitive {
				primitive,
				path,
				is_builtin,
			} => ConcreteType::Primitive {
				primitive: primitive.clone(),
				path: path.clone(),
				is_builtin: *is_builtin,
			},
			RustTypeTarget::Reference { .. } => {
				let java_class: &Rc<JavaClassName> = self
					.rust_to_java
					.get(rust_type)
					.expect("TypeMappings invariant violated: rust_type missing in rust_to_java map");
				ConcreteType::Object {
					name: java_class.clone(),
					reference_type: rust_type.clone(),
				}
			}
		})
	}

	/// Get the canonical Rust type path for a Java class name
	pub fn map_java_class_to_rust_type(&self, java_class: &JavaClassName) -> Option<&RustType> {
		self.java_to_rust.get(java_class).map(std::convert::AsRef::as_ref)
	}

	/// Get an iterator over all Java class names
	#[allow(
		unused,
		reason = "Expose the Java-class side of the internal mapping for generator extensions."
	)]
	pub fn java_classes(&self) -> impl Iterator<Item = &Rc<JavaClassName>> {
		self.java_to_rust.keys()
	}

	/// Get an iterator over all Rust types
	pub fn rust_types(&self) -> impl Iterator<Item = &Rc<RustType>> {
		self.rust_types.iter()
	}

	/// Get an iterator over `rust_path` -> `java_class` mappings
	pub fn rust_to_java_iter(&self) -> impl Iterator<Item = (&Rc<RustType>, &Rc<JavaClassName>)> {
		self.rust_to_java.iter()
	}

	/// Get the jni crate path
	pub const fn jni_crate(&self) -> &syn::Path {
		&self.jni_crate
	}

	/// Check if a Java class is a core type that cannot normally be bound
	pub fn is_core_java_type(&self, java_class: &JavaClassName) -> bool {
		self.core_java.contains(java_class)
	}
}
