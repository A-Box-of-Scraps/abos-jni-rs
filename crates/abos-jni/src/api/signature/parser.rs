use super::{Error, JNIString, JavaType, Primitive, Result, RuntimeFieldSignature, RuntimeMethodSignature};

/// Parse a primitive type descriptor from the start of `input`.
/// Returns the parsed `Primitive` and the remaining input.
pub(super) fn parse_primitive(input: &str) -> Result<(Primitive, &str)> {
	let (&first, _rest) = input
		.as_bytes()
		.split_first()
		.ok_or_else(|| Error::ParseFailed("unexpected end of input".into()))?;

	let prim: Primitive = match first {
		b'Z' => Primitive::Boolean,
		b'B' => Primitive::Byte,
		b'C' => Primitive::Char,
		b'D' => Primitive::Double,
		b'F' => Primitive::Float,
		b'I' => Primitive::Int,
		b'J' => Primitive::Long,
		b'S' => Primitive::Short,
		b'V' => Primitive::Void,
		_ => {
			return Err(Error::ParseFailed(format!(
				"expected primitive type, got '{}'",
				first as char
			)));
		}
	};

	// SAFETY: we split one ASCII byte, rest is valid UTF-8 at that offset
	Ok((prim, &input[1..]))
}

/// Parse a non-void primitive type descriptor.
pub(super) fn parse_non_void_primitive(input: &str) -> Result<(Primitive, &str)> {
	let (prim, rest) = parse_primitive(input)?;
	if matches!(prim, Primitive::Void) {
		return Err(Error::ParseFailed("void is not valid in this position".into()));
	}
	Ok((prim, rest))
}

/// Parse an array type descriptor (`[<element_type>`).
pub(super) fn parse_array(input: &str) -> Result<(JavaType, &str)> {
	let input: &str = input
		.strip_prefix('[')
		.ok_or_else(|| Error::ParseFailed("expected '[' for array type".into()))?;
	let (_, rest) = parse_non_void_type(input)?;
	Ok((JavaType::Array, rest))
}

/// Parse an object type descriptor (`Lclassname;`).
pub(super) fn parse_object(input: &str) -> Result<(JavaType, &str)> {
	// JVMS §4.2.2: unqualified names must not contain '.', ';', '[', or '/'
	pub(super) const fn is_unqualified(c: char) -> bool {
		!matches!(c, '.' | ';' | '[' | '/')
	}

	let input: &str = input
		.strip_prefix('L')
		.ok_or_else(|| Error::ParseFailed("expected 'L' for object type".into()))?;

	let bytes: &[u8] = input.as_bytes();
	let mut i = 0;
	// Must start with at least one unqualified char (no leading '/')
	if i >= bytes.len() || !is_unqualified(bytes[i] as char) {
		return Err(Error::ParseFailed("expected class name after 'L'".into()));
	}

	while i < bytes.len() && is_unqualified(bytes[i] as char) {
		i += 1;
	}

	// Optionally followed by ('/' segment)* where each segment is non-empty
	while i < bytes.len() && bytes[i] == b'/' {
		i += 1; // consume '/'
		let seg_start: usize = i;
		while i < bytes.len() && is_unqualified(bytes[i] as char) {
			i += 1;
		}

		if i == seg_start {
			return Err(Error::ParseFailed("expected class name segment after '/'".into()));
		}
	}

	// Must end with ';'
	if i >= bytes.len() || bytes[i] != b';' {
		return Err(Error::ParseFailed("expected ';' to close object type".into()));
	}

	Ok((JavaType::Object, &input[i + 1..]))
}

/// Parse any type descriptor (primitive, object, or array), including void.
pub(super) fn parse_type(input: &str) -> Result<(JavaType, &str)> {
	match input.as_bytes().first() {
		Some(b'L') => parse_object(input),
		Some(b'[') => parse_array(input),
		Some(_) => {
			let (prim, rest) = parse_primitive(input)?;
			Ok((JavaType::Primitive(prim), rest))
		}
		None => Err(Error::ParseFailed("unexpected end of input".into())),
	}
}

/// Parse any non-void type descriptor (primitive, object, or array).
pub(super) fn parse_non_void_type(input: &str) -> Result<(JavaType, &str)> {
	match input.as_bytes().first() {
		Some(b'L') => parse_object(input),
		Some(b'[') => parse_array(input),
		Some(_) => {
			let (prim, rest) = parse_non_void_primitive(input)?;
			Ok((JavaType::Primitive(prim), rest))
		}
		None => Err(Error::ParseFailed("unexpected end of input".into())),
	}
}

/// Parse argument types enclosed in parentheses (`(<type>*)`).
pub(super) fn parse_args(input: &str) -> Result<(Vec<JavaType>, &str)> {
	let input: &str = input
		.strip_prefix('(')
		.ok_or_else(|| Error::ParseFailed("expected '(' to start arguments".into()))?;

	let mut args: Vec<JavaType> = Vec::new();
	let mut rest: &str = input;
	loop {
		match rest.as_bytes().first() {
			Some(b')') => return Ok((args, &rest[1..])),
			Some(_) => {
				let (ty, remaining) = parse_non_void_type(rest)?;
				args.push(ty);
				rest = remaining;
			}
			None => {
				return Err(Error::ParseFailed("unexpected end of input, expected ')'".into()));
			}
		}
	}
}

/// Note: we initially return a placeholder signature string, which is replaced later
pub(super) fn parse_method_sig(input: &str) -> Result<(RuntimeMethodSignature, &str)> {
	let (args, rest) = parse_args(input)?;
	let (return_type, rest) = parse_type(rest)?;
	Ok((
		RuntimeMethodSignature {
			sig: JNIString::new(""),
			args,
			ret: return_type,
		},
		rest,
	))
}

/// Note: we initially return a placeholder signature string, which is replaced later
pub(super) fn parse_field_sig(input: &str) -> Result<(RuntimeFieldSignature, &str)> {
	let (ty, rest) = parse_non_void_type(input)?;
	Ok((
		RuntimeFieldSignature {
			sig: JNIString::new(""),
			ty,
		},
		rest,
	))
}
