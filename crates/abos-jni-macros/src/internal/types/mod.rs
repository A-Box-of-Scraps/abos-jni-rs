use std::rc::Rc;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
	Ident, Result, Token, braced, custom_keyword, parenthesized,
	parse::{Parse, ParseStream},
	token,
};

use crate::internal::str::lit_cstr_mutf8;

custom_keyword!(typealias);

custom_keyword!(Always);
custom_keyword!(UnsafeDebugOnly);
custom_keyword!(UnsafeNever);

/// Format a syn::Path as a string without spaces (for rustdoc links)
pub fn path_to_string_no_spaces(path: &syn::Path) -> String {
	quote!(#path).to_string().replace(" ", "")
}

/// Represents a primitive Java type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
	Void,
	Boolean,
	Byte,
	Char,
	Short,
	Int,
	Long,
	Float,
	Double,
}

impl PrimitiveType {
	/// Convert primitive type to JNI descriptor character
	pub fn to_jni_descriptor(&self) -> &'static str {
		match self {
			PrimitiveType::Void => "V",
			PrimitiveType::Boolean => "Z",
			PrimitiveType::Byte => "B",
			PrimitiveType::Char => "C",
			PrimitiveType::Short => "S",
			PrimitiveType::Int => "I",
			PrimitiveType::Long => "J",
			PrimitiveType::Float => "F",
			PrimitiveType::Double => "D",
		}
	}
}

/// Represents a Java class name (package + class)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaClassName {
	/// Package segments (e.g., ["java", "lang"])
	pub package: Vec<String>,
	/// Class name (including optional inner class names separated by '$')
	pub class: String,
}

impl JavaClassName {
	/// Convert to JNI internal form (e.g., "java/lang/String" or "java/lang/Outer$Inner")
	pub fn to_jni_internal(&self) -> String {
		let mut result = String::new();

		// Add package
		for (i, segment) in self.package.iter().enumerate() {
			if i > 0 {
				result.push('/');
			}
			result.push_str(segment);
		}

		// Add class name
		if !self.package.is_empty() {
			result.push('/');
		}
		result.push_str(&self.class);

		result
	}

	/// Convert to Java dotted form (e.g., "java.lang.String" or "java.lang.Outer$Inner")
	pub fn to_java_dotted(&self) -> String {
		let mut result = String::new();

		// Add package
		for (i, segment) in self.package.iter().enumerate() {
			if i > 0 {
				result.push('.');
			}
			result.push_str(segment);
		}

		// Add class name
		if !self.package.is_empty() {
			result.push('.');
		}
		result.push_str(&self.class);

		result
	}

	/// Convert to JNI object descriptor (e.g., "Ljava/lang/String;")
	pub fn to_jni_descriptor(&self) -> String {
		format!("L{};", self.to_jni_internal())
	}
}

impl Parse for JavaClassName {
	/// Parse a Java class name from a ParseStream or string literal
	/// Supports:
	/// - Dotted package.Class syntax (e.g., `java.lang.String`)
	/// - Inner classes with :: separator (e.g., `java.lang.Outer::Inner`)
	/// - String literals with dotted syntax
	/// - Special case for default-package classes starting with `.` (e.g., `.NoPackage`)
	///
	/// The parser requires at least one dot in the input to differentiate from Rust types,
	/// except for the special default-package case which starts with a `.`
	fn parse(input: ParseStream) -> Result<Self> {
		// Check if it's a string literal
		if input.peek(syn::LitStr) {
			let lit = input.parse::<syn::LitStr>()?;
			let class_str = lit.value();

			// Split by dots
			let parts: Vec<&str> = class_str.split('.').collect();
			if parts.is_empty() || (parts.len() == 1 && !class_str.starts_with('.')) {
				return Err(syn::Error::new(
					lit.span(),
					"Java class name in string literal must contain at least one dot (e.g., \"java.lang.String\" or \".NoPackage\" for default package)",
				));
			}

			let class = parts.last().unwrap().to_string();
			let package: Vec<String> = if class_str.starts_with('.') {
				// Default package case
				Vec::new()
			} else {
				parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect()
			};

			return Ok(JavaClassName { package, class });
		}

		// Check for dot-prefixed default package class (e.g., .NoPackage)
		if input.peek(Token![.]) {
			input.parse::<Token![.]>()?;
			let mut class_name = input.parse::<Ident>()?.to_string();

			// Parse inner classes if any and concatenate with $
			while input.peek(Token![::]) {
				input.parse::<Token![::]>()?;
				let inner = input.parse::<Ident>()?;
				class_name.push('$');
				class_name.push_str(&inner.to_string());
			}

			return Ok(JavaClassName {
				package: Vec::new(),
				class: class_name,
			});
		}

		// Check if we have an ident followed by a dot (using lookahead to avoid consuming the ident)
		// This ensures we don't consume a token that might be a primitive type or Rust type
		if input.peek(Ident) && input.peek2(Token![.]) {
			// Parse as ident-based syntax: package.Class
			let mut segments = Vec::new();
			let first = input.parse::<Ident>()?;
			segments.push(first.to_string());

			// Parse dot-separated package/class segments
			while input.peek(Token![.]) {
				input.parse::<Token![.]>()?;
				let segment = input.parse::<Ident>()?;
				segments.push(segment.to_string());
			}

			// Parse inner classes (separated by ::) and concatenate with $
			let mut inner_class_parts = Vec::new();
			while input.peek(Token![::]) {
				input.parse::<Token![::]>()?;
				let inner = input.parse::<Ident>()?;
				inner_class_parts.push(inner.to_string());
			}

			// Build the class name: last segment + any inner classes joined with $
			let mut class = segments.pop().unwrap();
			if !inner_class_parts.is_empty() {
				class.push('$');
				class.push_str(&inner_class_parts.join("$"));
			}

			return Ok(JavaClassName {
				package: segments,
				class,
			});
		}

		// None of the valid patterns matched
		Err(syn::Error::new(
			input.span(),
			"Expected Java class name: either a string literal with dots (e.g., \"java.lang.String\"), a dotted identifier (e.g., java.lang.String), or a dot-prefixed class for default package (e.g., .NoPackage)",
		))
	}
}
/// Represents a type in the signature
#[derive(Debug, Clone, PartialEq)]
pub enum SigType {
	/// A type alias that must be mapped via TypeMappings
	/// into a Java class name or a primitive type
	Alias(String),
	/// Object type (Java class)
	Object(JavaClassName),
	/// Array type with element type and dimensions
	Array(Box<SigType>, usize),
}

impl SigType {
	/// Convert to JNI descriptor
	pub fn to_jni_descriptor(&self, type_mappings: &TypeMappings) -> Result<String> {
		match self {
			SigType::Alias(name) => match type_mappings.map_alias(name) {
				Some(ConcreteType::Primitive { primitive, .. }) => Ok(primitive.to_jni_descriptor().to_string()),
				Some(ConcreteType::Object { name: java_class, .. }) => Ok(java_class.to_jni_descriptor()),
				None => Err(syn::Error::new(Span::call_site(), format!("Unknown type '{}'", name))),
			},
			SigType::Object(class) => Ok(class.to_jni_descriptor()),
			SigType::Array(elem, dims) => {
				let elem_desc = elem.to_jni_descriptor(type_mappings)?;
				let mut result = String::new();
				for _ in 0..*dims {
					result.push('[');
				}
				result.push_str(&elem_desc);
				Ok(result)
			}
		}
	}

	pub fn try_as_primitive(&self, type_mappings: &TypeMappings) -> Option<PrimitiveType> {
		match self {
			SigType::Alias(name) => match type_mappings.map_alias(name) {
				Some(ConcreteType::Primitive { primitive, .. }) => Some(primitive),
				_ => None,
			},
			_ => None,
		}
	}
}

/// Parse a type (can be primitive, object, rust reference, or array)
pub fn parse_type(input: ParseStream, type_mappings: &TypeMappings) -> Result<SigType> {
	// Check for leading reference operator (ignore it)
	if input.peek(Token![&]) {
		input.parse::<Token![&]>()?;
	}

	// Check for array syntax [...]
	if input.peek(token::Bracket) {
		let content;
		let bracket_span = input.span();
		syn::bracketed!(content in input);
		let elem_type = parse_type(&content, type_mappings)?;

		// Validate that the element type can be used in an array
		validate_array_element_type(&elem_type, type_mappings, bracket_span)?;

		// Count array dimensions
		let mut dims = 1;
		while input.peek(token::Bracket) {
			let _content;
			syn::bracketed!(_content in input);
			// Nested array - increment dimensions
			dims += 1;
		}

		return Ok(SigType::Array(Box::new(elem_type), dims));
	}

	// Check for unit type ()
	if input.peek(token::Paren) {
		let content;
		parenthesized!(content in input);
		if content.is_empty() {
			return Ok(SigType::Alias("void".to_string()));
		} else {
			return Err(syn::Error::new(
				content.span(),
				"Expected empty parentheses for void type",
			));
		}
	}

	// Try to parse as Java class name first (handles string literals, .NoPackage, and package.Class)
	// This is done speculatively - if it fails, we'll try other interpretations
	let base_type = if let Ok(java_class) = input.parse::<JavaClassName>() {
		SigType::Object(java_class)
	} else {
		// Not a Java class name, so it must be a TypeMappings type (could be
		// primitive lie jint/i32 or Reference like
		// JString/abos_jni::objects::JString)
		let path = input.parse::<syn::Path>()?;
		let path_str = path_to_string_no_spaces(&path);

		SigType::Alias(path_str)
	};

	// Check for suffix array syntax (applies to all base types)
	let mut dims = 0;
	let mut first_bracket_span = None;
	while input.peek(token::Bracket) {
		let bracket_span = input.span();
		if first_bracket_span.is_none() {
			first_bracket_span = Some(bracket_span);
		}
		let content;
		syn::bracketed!(content in input);
		if !content.is_empty() {
			return Err(syn::Error::new(
				content.span(),
				"Expected empty brackets for array suffix syntax",
			));
		}
		dims += 1;
	}

	// If we have array dimensions, validate and wrap in Array type
	if dims > 0 {
		// Validate that the element type can be used in an array (void cannot)
		validate_array_element_type(&base_type, type_mappings, first_bracket_span.unwrap())?;
		Ok(SigType::Array(Box::new(base_type), dims))
	} else {
		Ok(base_type)
	}
}

#[derive(Clone, Debug, Eq)]
enum RustTypeTarget {
	/// A primitive type that maps to a `abos_jni::sys` type
	Primitive {
		/// Is this a standard primitive type or a user-defined mapping?
		is_builtin: bool,
		/// The primitive type
		primitive: PrimitiveType,
		/// The name or full path for the primitive type (e.g., "i32" or "abos_jni::sys::jint")
		///
		/// If the jni crate is renamed, this path will reflect the renamed crate path.
		///
		/// If this represents a Rust primitive type, such as `i32`, then this will match
		/// that type name.
		path: String,
	},
	Reference {
		/// Is this a built-in `jni` crate Reference type?
		is_builtin: bool,
		/// The full path for a `jni` `Reference` type (e.g., "abos_jni::objects::JString")
		///
		/// If the jni crate is renamed, this path will reflect the renamed crate path.
		path: String,
	},
}

// Ignore the 'is_builtin' field for equality and hashing

impl PartialEq for RustTypeTarget {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(
				RustTypeTarget::Primitive {
					primitive: p1,
					path: path1,
					..
				},
				RustTypeTarget::Primitive {
					primitive: p2,
					path: path2,
					..
				},
			) => p1 == p2 && path1 == path2,
			(RustTypeTarget::Reference { path: path1, .. }, RustTypeTarget::Reference { path: path2, .. }) => {
				path1 == path2
			}
			_ => false,
		}
	}
}
impl std::hash::Hash for RustTypeTarget {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		match &self {
			RustTypeTarget::Primitive { primitive, path, .. } => {
				primitive.hash(state);
				path.hash(state);
			}
			RustTypeTarget::Reference { path, .. } => {
				path.hash(state);
			}
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RustType {
	target: RustTypeTarget,
}

impl RustType {
	pub fn primitive(&self) -> Option<PrimitiveType> {
		if let RustTypeTarget::Primitive { primitive, .. } = &self.target {
			Some(primitive.clone())
		} else {
			None
		}
	}
	pub fn is_builtin(&self) -> bool {
		match self.target {
			RustTypeTarget::Primitive { is_builtin, .. } => is_builtin,
			RustTypeTarget::Reference { is_builtin, .. } => is_builtin,
		}
	}
	pub fn path(&self) -> &str {
		match &self.target {
			RustTypeTarget::Primitive { path, .. } => path.as_str(),
			RustTypeTarget::Reference { path, .. } => path.as_str(),
		}
	}
}

pub enum ConcreteType {
	Primitive {
		primitive: PrimitiveType,
		/// The full path for the primitive type (e.g., "abos_jni::sys::jint")
		///
		/// If the jni crate is renamed, this path will reflect the renamed
		/// crate path.
		///
		/// If the lookup was done using a Rust type alias such as `i32` then
		/// this will correspond to that alias.
		///
		/// If the lookup was done using a Java type alias such as `int`, then
		/// this will correspond to the canonical `abos_jni::sys` type path.
		path: String,
		/// Whether this is a built-in primitive type mapping or a user-defined one
		is_builtin: bool,
	},
	Object {
		name: Rc<JavaClassName>,
		reference_type: Rc<RustType>,
	},
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum AbiCheck {
	#[default]
	Always,
	UnsafeDebugOnly,
	UnsafeNever,
}

impl AbiCheck {
	pub fn requires_abi_check(&self) -> bool {
		match self {
			AbiCheck::Always => true,
			AbiCheck::UnsafeDebugOnly => cfg!(debug_assertions),
			AbiCheck::UnsafeNever => false,
		}
	}
}

impl Parse for AbiCheck {
	fn parse(input: ParseStream) -> Result<Self> {
		let lookahead = input.lookahead1();
		if lookahead.peek(Always) {
			input.parse::<Always>()?;
			Ok(AbiCheck::Always)
		} else if lookahead.peek(UnsafeDebugOnly) {
			input.parse::<UnsafeDebugOnly>()?;
			Ok(AbiCheck::UnsafeDebugOnly)
		} else if lookahead.peek(UnsafeNever) {
			input.parse::<UnsafeNever>()?;
			Ok(AbiCheck::UnsafeNever)
		} else {
			Err(lookahead.error())
		}
	}
}

mod mappings;
pub use mappings::*;
mod conversion;
pub use conversion::*;
