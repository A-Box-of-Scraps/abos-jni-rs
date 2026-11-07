//! Procedural macro for generating JNI signatures at compile time
//!
//! This module provides a `jni_sig!` macro that parses method and field signatures
//! and generates JNI signature string literals.
//!
//! # Method Signatures
//! Method signatures have the form `(parameters) -> return_type` and generate a JNI
//! method descriptor like `"(ILjava/lang/String;)V"`.
//!
//! # Field Signatures
//! Field signatures are just a bare type and generate a JNI field descriptor like
//! `"Ljava/lang/String;"` or `"I"`.
//!
//! The parser automatically detects which type of signature based on the presence of
//! parentheses and a return arrow.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
	Ident, Result, Token, custom_keyword, parenthesized,
	parse::{Parse, ParseStream},
	token,
};

use crate::internal::{
	str::lit_cstr_mutf8,
	types::{ConcreteType, JavaClassName, PrimitiveType, SigType, TypeMappings, parse_type},
};

custom_keyword!(sig);

/// Represents a method parameter
#[derive(Debug, Clone)]
pub struct Parameter {
	pub name: Ident,
	pub ty: SigType,
}

/// Represents a method signature
#[derive(Debug, Clone)]
pub struct MethodSignature {
	pub parameters: Vec<Parameter>,
	pub return_type: SigType,
}

impl MethodSignature {
	/// Convert to JNI method signature string
	pub fn to_jni_signature(&self, type_mappings: &TypeMappings) -> Result<String> {
		let mut result: String = String::from("(");

		for param in &self.parameters {
			result.push_str(&param.ty.to_jni_descriptor(type_mappings)?);
		}

		result.push(')');
		result.push_str(&self.return_type.to_jni_descriptor(type_mappings)?);

		Ok(result)
	}
}

/// Parse sig = (args) -> ret
pub fn parse_method_sig(input: ParseStream, type_mappings: &TypeMappings) -> Result<MethodSignature> {
	input.parse::<sig>()?;
	input.parse::<Token![=]>()?;

	let args_content;
	parenthesized!(args_content in input);

	let mut parameters: Vec<Parameter> = Vec::new();
	let mut param_index = 0;

	while !args_content.is_empty() {
		let param: Parameter = parse_parameter_with_index(&args_content, param_index, type_mappings)?;
		parameters.push(param);
		param_index += 1;

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

	Ok(MethodSignature {
		parameters,
		return_type,
	})
}

/// Represents a field signature
#[derive(Debug, Clone)]
pub struct FieldSignature {
	pub field_type: SigType,
}

impl FieldSignature {
	/// Convert to JNI field signature string (just the type descriptor)
	pub fn to_jni_signature(&self, type_mappings: &TypeMappings) -> Result<String> {
		self.field_type.to_jni_descriptor(type_mappings)
	}
}

/// Parse sig = Type for fields
pub fn parse_field_sig(input: ParseStream, type_mappings: &TypeMappings) -> Result<FieldSignature> {
	input.parse::<sig>()?;
	input.parse::<Token![=]>()?;
	let field_type: SigType = parse_type(input, type_mappings)?;
	Ok(FieldSignature { field_type })
}

/// Represents either a method or field signature
#[derive(Debug, Clone)]
pub enum Signature {
	Method(MethodSignature),
	Field(FieldSignature),
}

impl Signature {
	/// Convert to JNI signature string
	pub fn to_jni_signature(&self, type_mappings: &TypeMappings) -> Result<String> {
		match self {
			Self::Method(sig) => sig.to_jni_signature(type_mappings),
			Self::Field(sig) => sig.to_jni_signature(type_mappings),
		}
	}
}

/// Parse a parameter with an index for fallback naming: `name: type` or just `type`
/// If only a type is provided, generates a fallback name like "arg0", "arg1", etc.
pub fn parse_parameter_with_index(input: ParseStream, index: usize, type_mappings: &TypeMappings) -> Result<Parameter> {
	if input.peek(Ident) && input.peek2(Token![:]) {
		let name: Ident = input.parse::<Ident>()?;
		input.parse::<Token![:]>()?;
		let ty: SigType = parse_type(input, type_mappings)?;
		Ok(Parameter { name, ty })
	} else {
		let ty: SigType = parse_type(input, type_mappings)?;
		let name = Ident::new(&format!("arg{index}"), Span::call_site());
		Ok(Parameter { name, ty })
	}
}

/// Represents the full macro input with named properties
struct SignatureInput {
	signature: Signature,
	type_mappings: TypeMappings,
}

mod expand;
mod parse;
pub use expand::*;
#[cfg(test)]
#[path = "../../../tests/unit/internal/signature.rs"]
mod tests;
