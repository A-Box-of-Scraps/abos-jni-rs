use super::{
	ConcreteType, Ident, JavaClassName, ParseStream, PrimitiveType, Rc, Result, RustType, RustTypeTarget, Span, Token,
	TypeMappings, braced, path_to_string_no_spaces, typealias,
};

impl TypeMappings {
	/// Insert a mapping from a Rust `Reference` type path to a Java class
	///
	/// If there is already a a Rust type mapping to the same Java class, the reverse
	/// mapping from Java class to Rust type path will be updated to point to the new
	/// Rust type path (so that user-defined types can override built-in types).
	///
	/// Note:
	/// - User code cannot create mappings for core Java classes.
	/// - User code cannot _change_ the mapping for an existing Rust type path.
	/// - User code cannot currently create multiple alias for a single Rust type path.
	///
	/// Returns an error if:
	/// - Attempting to map a core Java class
	/// - Attempting to change the mapping for an existing Rust type path
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

		// Check if this is a core Java class that cannot be remapped
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

		// Check if this alias already has a mapping to a different Rust type
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

		// Insert the mappings
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

	/// Parse `type_map` block with Reference + primitive mappings and aliases
	///
	/// This is a shared parser used by both `bind_java_type`! and `jni_sig`! macros. Parses a
	/// braced block containing type mappings from Rust type paths to Java class names.
	///
	/// # Syntax
	///
	/// A `type_map` block can be comprised of:
	/// - Reference type mappings
	/// - Primitive type mappings (unsafe)
	/// - Type aliases
	///
	/// In all cases:
	/// - The type separator is `=>`
	/// - Mappings are separated by commas
	/// - Trailing comma is optional
	///
	/// ## Reference Type Mappings
	///
	/// Each Reference type mapping has the form: `RustPath => java.lang.Class`
	/// - `RustPath` can be a simple identifier (e.g., `JString`) or a path (e.g.,
	///   `abos_jni::objects::JString`)
	///
	/// ## Primitive Type Mappings
	///
	/// Primitive type mappings are considered `unsafe`, but can be especially useful for mapping
	/// wrapped pointers types to a Java `long` type (e.g. to associate native handles with Java
	/// objects).
	///
	/// Each Primitive type mapping has the form: `unsafe RustPrimitive => java_primitive`
	///
	/// ## Aliases
	///
	/// Type aliases allow you to create a new name for an existing type mapping. This can be useful
	/// for making signatures more readable (e.g. before you have a defined a real type binding).
	///
	/// An alias mapping has the form: `alias NewTypeName => ExistingTypeName`
	///
	/// Aliases for array types are not supported.
	///
	/// # Example
	/// ```ignore
	/// type_map = {
	///     MyType => java.lang.MyType,
	///     custom::MyOtherType => com.example.MyOtherType,
	///     unsafe MyBoxHandle => long,
	///     typealias MyTypeAlias => MyType,
	///     typealias MyOtherTypeAlias => JObject,
	/// }
	/// ```
	pub fn parse_mappings(&mut self, input: ParseStream) -> Result<()> {
		// Optional '=' before block
		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
		}

		let mappings_content;
		braced!(mappings_content in input);

		while !mappings_content.is_empty() {
			self.parse_mapping(&mappings_content)?;
			// Require comma between entries, but trailing comma is optional.
			if !mappings_content.is_empty() {
				mappings_content.parse::<Token![,]>()?;
			}
		}

		Ok(())
	}

	fn parse_mapping(&mut self, input: ParseStream) -> Result<()> {
		if input.peek(typealias) {
			// Parse alias mapping
			input.parse::<typealias>()?;

			// Parse new alias name
			let new_alias: syn::Path = input.parse()?;

			// Parse separator
			input.parse::<Token![=>]>()?;

			// Parse existing type name
			let existing_type: syn::Path = input.parse()?;

			// Insert alias
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

			// Parse Rust path
			let rust_path: syn::Path = input.parse()?;

			// Convert path to string without spaces
			let rust_path_str: String = path_to_string_no_spaces(&rust_path);

			// Parse separator
			input.parse::<Token![=>]>()?;

			if is_prim_mapping {
				self.parse_primitive_mapping(input, &rust_path_str)?;
			} else {
				// Parse Java class name
				let java_class_mapping: JavaClassName = input.parse()?;

				// Insert mapping
				self.insert_ref_type(&rust_path_str, &rust_path_str, java_class_mapping, false)?;
			}
		}
		Ok(())
	}

	fn parse_primitive_mapping(&mut self, input: ParseStream, rust_path_str: &str) -> Result<()> {
		// Parse primitive type name
		let prim_type_ident: Ident = input.parse()?;
		let prim_type_name: String = prim_type_ident.to_string();

		// Validate type name by checking that it's a "built-in" primitive type name
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
