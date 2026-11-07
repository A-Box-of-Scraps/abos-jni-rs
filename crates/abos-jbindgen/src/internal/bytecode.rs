use crate::{ArgInfo, ClassInfo, Error, FieldInfo, InstanceOfInfo, MethodInfo, MethodSignature, Result, TypeInfo};
use abos_cafebabe::{AccessFlags, ClassFile};

fn type_prefix(descriptor: &str, allow_void: bool) -> Result<(TypeInfo, &str)> {
	let dimensions: usize = descriptor.bytes().take_while(|byte| *byte == b'[').count();
	let element: &str = &descriptor[dimensions..];
	let (name, rest, primitive) = if let Some(object) = element.strip_prefix('L') {
		let (name, rest) = object
			.split_once(';')
			.ok_or_else(|| Error::Parse("unterminated object descriptor".into()))?;
		if name.is_empty() {
			return Err(Error::Parse("empty object descriptor".into()));
		}
		(name.replace('/', "."), rest, false)
	} else {
		let name: &str = match element.as_bytes().first() {
			Some(b'Z') => "boolean",
			Some(b'B') => "byte",
			Some(b'C') => "char",
			Some(b'S') => "short",
			Some(b'I') => "int",
			Some(b'J') => "long",
			Some(b'F') => "float",
			Some(b'D') => "double",
			Some(b'V') if allow_void && dimensions == 0 => "void",
			_ => return Err(Error::Parse(format!("invalid type descriptor: {descriptor}"))),
		};
		(name.to_owned(), &element[1..], true)
	};
	Ok((
		TypeInfo {
			name,
			array_dimensions: dimensions,
			is_primitive: primitive,
		},
		rest,
	))
}

/// Parse a complete non-void JVM type descriptor.
///
/// # Errors
/// Returns an error for an unsupported or incomplete descriptor, or trailing input.
pub fn parse_type_descriptor(descriptor: &str) -> Result<TypeInfo> {
	let (ty, rest) = type_prefix(descriptor, false)?;
	if !rest.is_empty() {
		return Err(Error::Parse(format!("trailing type descriptor: {rest}")));
	}
	Ok(ty)
}

/// Parse argument and return types from a complete JVM method descriptor.
///
/// # Errors
/// Returns an error for invalid arguments, return types, delimiters, or trailing input.
pub fn parse_method_descriptor(descriptor: &str) -> Result<(Vec<ArgInfo>, TypeInfo)> {
	let mut remaining: &str = descriptor
		.strip_prefix('(')
		.ok_or_else(|| Error::Parse("missing method parameter list".into()))?;
	let mut arguments: Vec<ArgInfo> = Vec::new();
	while !remaining.starts_with(')') {
		let (type_info, rest) = type_prefix(remaining, false)?;
		arguments.push(ArgInfo {
			name: None,
			type_info,
			rust_primitive: None,
		});
		remaining = rest;
	}
	let (return_type, rest) = type_prefix(&remaining[1..], true)?;
	if !rest.is_empty() {
		return Err(Error::Parse(format!("trailing method descriptor: {rest}")));
	}
	Ok((arguments, return_type))
}

/// Extract binding metadata from a complete class file.
///
/// # Errors
/// Returns an error if class-file metadata or a selected member descriptor cannot be parsed.
pub fn parse_class(bytes: &[u8]) -> Result<ClassInfo> {
	let class: ClassFile = abos_cafebabe::parse_class(bytes)?;
	let name: String = class.this_class.clone();
	let (package, simple) = name.rsplit_once('/').unwrap_or(("", &name));
	let mut info = ClassInfo {
		class_name: name.clone(),
		package: package
			.split('.')
			.flat_map(|part| part.split('/'))
			.filter(|part| !part.is_empty())
			.map(str::to_owned)
			.collect(),
		simple_name: simple.to_owned(),
		documentation: None,
		rust_name_override: None,
		constructors: Vec::new(),
		methods: Vec::new(),
		fields: Vec::new(),
		native_methods: Vec::new(),
		instance_of: Vec::new(),
	};
	for field in &class.fields {
		if field.access_flags.contains(AccessFlags::PRIVATE) {
			continue;
		}
		info.fields.push(FieldInfo {
			name: field.name.clone(),
			documentation: None,
			rust_name_override: None,
			type_info: parse_type_descriptor(&field.descriptor)?,
			is_static: field.access_flags.contains(AccessFlags::STATIC),
			is_final: field.access_flags.contains(AccessFlags::FINAL),
			is_deprecated: false,
		});
	}
	for method in &class.methods {
		let name: String = method.name.clone();
		let native: bool = method.access_flags.contains(AccessFlags::NATIVE);
		if name == "<clinit>" || (!native && method.access_flags.contains(AccessFlags::PRIVATE)) {
			continue;
		}
		let (arguments, return_type) = parse_method_descriptor(&method.descriptor)?;
		let method_info = MethodInfo {
			is_constructor: name == "<init>",
			name,
			documentation: None,
			rust_name_override: None,
			signature: MethodSignature { arguments, return_type },
			is_static: method.access_flags.contains(AccessFlags::STATIC),
			is_native: native,
			is_deprecated: false,
			is_public: method.access_flags.contains(AccessFlags::PUBLIC),
		};
		if native {
			info.native_methods.push(method_info);
		} else if method_info.is_constructor {
			info.constructors.push(method_info);
		} else {
			info.methods.push(method_info);
		}
	}
	if let Some(parent) = &class.super_class
		&& &**parent != "java/lang/Object"
	{
		info.instance_of.push(InstanceOfInfo {
			java_type: parent.replace('/', "."),
			stem: None,
		});
	}
	for interface in &class.interfaces {
		info.instance_of.push(InstanceOfInfo {
			java_type: interface.replace('/', "."),
			stem: None,
		});
	}
	Ok(info)
}
