#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
//! Examples demonstrating the `jni_sig!` family of macros for creating JNI signatures.
//!
//! The `jni_sig!` macro and its variants provide compile-time parsing and validation
//! of JNI method and field signatures, with support for both readable Rust-like syntax
//! and raw JNI signature strings.
//!
//! # Output Variants
//!
//! - `jni_sig!` - Returns a typed `MethodSignature` or `FieldSignature` struct
//! - `jni_sig_str!` - Returns a `&str` literal
//! - `jni_sig_cstr!` - Returns a `&CStr` literal (MUTF-8 encoded)
//! - `jni_sig_jstr!` - Returns a `&'static JNIStr` (MUTF-8 encoded)

use abos_jni::signature::{FieldSignature, JavaType, MethodSignature, Primitive};
use abos_jni::strings::JNIStr;
use abos_jni_macros::{jni_sig, jni_sig_cstr, jni_sig_jstr, jni_sig_str};
use std::ffi::CStr;

// ============================================================================
// Method Signatures
// ============================================================================

fn method_signatures() {
	// Methods with Java objects
	const WITH_OBJECTS: MethodSignature = jni_sig!(
		(id: jint, name: java.lang.String) -> java.lang.Object
	);
	// No-argument methods
	const NO_ARGS: MethodSignature = jni_sig!(() -> jint);
	// Methods with arrays (prefix syntax)
	const ARRAY_PREFIX: MethodSignature = jni_sig!(
		(data: [jint], labels: [java.lang.String]) -> void
	);
	// Methods with arrays (suffix syntax)
	const ARRAY_SUFFIX: MethodSignature = jni_sig!(
		(grid: jint[][]) -> java.lang.String[]
	);
	// Methods with inner classes
	const INNER_CLASS: MethodSignature = jni_sig!(
		(state: java.lang.Thread::State) -> void
	);
	// Optional parameter names (types only)
	const UNNAMED_PARAMS: MethodSignature = jni_sig!(
		(jint, jboolean, java.lang.String) -> void
	);
	// Mixing named and unnamed parameters
	const MIXED_PARAMS: MethodSignature = jni_sig!(
		(jint, name: java.lang.String, jboolean) -> void
	);
	// Implicit void return (no return type specified)
	const IMPLICIT_VOID: MethodSignature = jni_sig!((a: jint));

	// Methods with primitive types
	// MethodSignature includes JavaType for each argument and the return type
	const SIMPLE: MethodSignature = jni_sig!((a: jint, b: jboolean) -> void);
	assert_method_signature(
		&SIMPLE,
		b"(IZ)V",
		&[
			JavaType::Primitive(Primitive::Int),
			JavaType::Primitive(Primitive::Boolean),
		],
		JavaType::Primitive(Primitive::Void),
	);
	assert_method_signature(
		&WITH_OBJECTS,
		b"(ILjava/lang/String;)Ljava/lang/Object;",
		&[JavaType::Primitive(Primitive::Int), JavaType::Object],
		JavaType::Object,
	);
	assert_method_signature(&NO_ARGS, b"()I", &[], JavaType::Primitive(Primitive::Int));
	assert_method_signature(
		&ARRAY_PREFIX,
		b"([I[Ljava/lang/String;)V",
		&[JavaType::Array, JavaType::Array],
		JavaType::Primitive(Primitive::Void),
	);
	assert_method_signature(
		&ARRAY_SUFFIX,
		b"([[I)[Ljava/lang/String;",
		&[JavaType::Array],
		JavaType::Array,
	);

	assert_eq!(INNER_CLASS.sig().to_bytes(), b"(Ljava/lang/Thread$State;)V");

	assert_eq!(UNNAMED_PARAMS.sig().to_bytes(), b"(IZLjava/lang/String;)V");

	assert_eq!(MIXED_PARAMS.sig().to_bytes(), b"(ILjava/lang/String;Z)V");

	assert_eq!(IMPLICIT_VOID.sig().to_bytes(), b"(I)V");
}

// ============================================================================
// Field Signatures
// ============================================================================

fn field_signatures() {
	const BOOLEAN_FIELD: FieldSignature = jni_sig!(jboolean);
	// Object fields
	const STRING_FIELD: FieldSignature = jni_sig!(java.lang.String);
	// Array fields (prefix syntax)
	const ARRAY_FIELD_PREFIX: FieldSignature = jni_sig!([jint]);
	// Array fields (suffix syntax)
	const ARRAY_FIELD_SUFFIX: FieldSignature = jni_sig!(java.lang.String[][]);
	// Inner class fields
	const INNER_CLASS_FIELD: FieldSignature = jni_sig!(java.lang.Thread::State);

	// Primitive fields
	// FieldSignature includes a JavaType for the field type
	const INT_FIELD: FieldSignature = jni_sig!(jint);
	assert_eq!(INT_FIELD.sig().to_bytes(), b"I");
	assert_eq!(INT_FIELD.ty(), JavaType::Primitive(Primitive::Int));

	assert_eq!(BOOLEAN_FIELD.sig().to_bytes(), b"Z");
	assert_eq!(BOOLEAN_FIELD.ty(), JavaType::Primitive(Primitive::Boolean));

	assert_eq!(STRING_FIELD.sig().to_bytes(), b"Ljava/lang/String;");
	assert_eq!(STRING_FIELD.ty(), JavaType::Object);

	assert_eq!(ARRAY_FIELD_PREFIX.sig().to_bytes(), b"[I");
	assert_eq!(ARRAY_FIELD_PREFIX.ty(), JavaType::Array);

	assert_eq!(ARRAY_FIELD_SUFFIX.sig().to_bytes(), b"[[Ljava/lang/String;");
	assert_eq!(ARRAY_FIELD_SUFFIX.ty(), JavaType::Array);

	assert_eq!(INNER_CLASS_FIELD.sig().to_bytes(), b"Ljava/lang/Thread$State;");
}

// ============================================================================
// Type Mappings
// ============================================================================

fn type_mappings() {
	// Type mappings with arrays
	const MAPPED_ARRAYS: MethodSignature = jni_sig!(
		type_map = {
			CustomType => com.example.CustomType,
		},
		(items: [CustomType]) -> [[CustomType]]
	);
	// Multiple type_map blocks (useful for wrapper macros)
	const MULTIPLE_MAPS: MethodSignature = jni_sig!(
		type_map = { TypeA => com.example.TypeA },
		type_map = { TypeB => com.example.TypeB },
		(a: TypeA, b: TypeB) -> void
	);
	// Named signature property with type mappings (order doesn't matter)
	const NAMED_SIG: MethodSignature = jni_sig!(
		type_map = { MyType => com.example.MyType },
		sig = (arg: MyType) -> void
	);
	// Built-in types (no mapping needed)
	const BUILTIN_TYPES: MethodSignature = jni_sig!(
		(obj: JObject, str: JString, class: JClass) -> JThrowable
	);
	// Type aliases
	const ALIASES: MethodSignature = jni_sig!(
		type_map = {
			CustomString => com.example.CustomString,
			typealias Str => CustomString,
			typealias Text => JString,
		},
		(a: Str, b: Text) -> void
	);
	// Unsafe primitive mappings (for handles/pointers passed as long)
	const UNSAFE_MAPPING: MethodSignature = jni_sig!(
		type_map = {
			unsafe NativeHandle => long,
		},
		(handle: NativeHandle) -> void
	);

	// Custom type mappings allow using Rust type names in signatures
	const CUSTOM_TYPES: MethodSignature = jni_sig!(
		type_map = {
			UserId => com.example.UserId,
			UserProfile => com.example.UserProfile,
		},
		(id: UserId) -> UserProfile
	);
	assert_eq!(
		CUSTOM_TYPES.sig().to_bytes(),
		b"(Lcom/example/UserId;)Lcom/example/UserProfile;"
	);

	assert_eq!(
		MAPPED_ARRAYS.sig().to_bytes(),
		b"([Lcom/example/CustomType;)[[Lcom/example/CustomType;"
	);

	assert_eq!(
		MULTIPLE_MAPS.sig().to_bytes(),
		b"(Lcom/example/TypeA;Lcom/example/TypeB;)V"
	);

	assert_eq!(NAMED_SIG.sig().to_bytes(), b"(Lcom/example/MyType;)V");

	assert_eq!(
		BUILTIN_TYPES.sig().to_bytes(),
		b"(Ljava/lang/Object;Ljava/lang/String;Ljava/lang/Class;)Ljava/lang/Throwable;"
	);

	assert_eq!(
		ALIASES.sig().to_bytes(),
		b"(Lcom/example/CustomString;Ljava/lang/String;)V"
	);

	assert_eq!(UNSAFE_MAPPING.sig().to_bytes(), b"(J)V");
}

// ============================================================================
// Output Variants
// ============================================================================

fn output_variants() {
	// jni_sig! returns a typed FieldSignature struct
	const FIELD: FieldSignature = jni_sig!(jint);
	// jni_sig_str! returns a plain &str
	const METHOD_STR: &str = jni_sig_str!((a: jint) -> java.lang.String);
	const FIELD_STR: &str = jni_sig_str!(jint);
	// jni_sig_cstr! returns a &CStr (MUTF-8 encoded)
	const METHOD_CSTR: &CStr = jni_sig_cstr!((a: jint) -> java.lang.String);
	const FIELD_CSTR: &CStr = jni_sig_cstr!(jint);
	// jni_sig_jstr! returns a &'static JNIStr (MUTF-8 encoded)
	const METHOD_JSTR: &JNIStr = jni_sig_jstr!((a: jint) -> java.lang.String);
	const FIELD_JSTR: &JNIStr = jni_sig_jstr!(jint);
	// All variants accept the same input syntax
	const COMPLEX_JSTR: &JNIStr = jni_sig_jstr!(
		type_map = { MyType => com.example.Type },
		([MyType], jint) -> java.lang.String[]
	);

	// jni_sig! returns a typed MethodSignature struct
	const METHOD: MethodSignature = jni_sig!((a: jint) -> java.lang.String);
	assert_eq!(METHOD.sig().to_bytes(), b"(I)Ljava/lang/String;");
	assert_eq!(METHOD.args().len(), 1);
	assert_eq!(METHOD.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(METHOD.ret(), JavaType::Object);

	assert_eq!(FIELD.sig().to_bytes(), b"I");
	assert_eq!(FIELD.ty(), JavaType::Primitive(Primitive::Int));

	assert_eq!(METHOD_STR, "(I)Ljava/lang/String;");

	assert_eq!(FIELD_STR, "I");

	assert_eq!(METHOD_CSTR.to_bytes(), b"(I)Ljava/lang/String;");

	assert_eq!(FIELD_CSTR.to_bytes(), b"I");

	assert_eq!(METHOD_JSTR.to_bytes(), b"(I)Ljava/lang/String;");

	assert_eq!(FIELD_JSTR.to_bytes(), b"I");

	assert_eq!(COMPLEX_JSTR.to_bytes(), b"([Lcom/example/Type;I)[Ljava/lang/String;");
}

// ============================================================================
// Raw JNI Signatures
// ============================================================================

fn raw_jni_signatures() {
	// Raw JNI field signatures
	// The parser resolves the JavaType for the field
	const RAW_FIELD: FieldSignature = jni_sig!("Ljava/lang/String;");
	// Raw signatures work with all output variants
	const RAW_STR: &str = jni_sig_str!("([I)[[Ljava/lang/Object;");
	const RAW_CSTR: &CStr = jni_sig_cstr!("[B");
	// Java inner classes with dollar sign
	const RAW_INNER: MethodSignature = jni_sig!("(Ljava/lang/Thread$State;)V");

	// Raw JNI method signatures (validated at compile time)
	// The parser resolves JavaType for each argument and the return type
	const RAW_METHOD: MethodSignature = jni_sig!("(ILjava/lang/String;)V");
	assert_eq!(RAW_METHOD.sig().to_bytes(), b"(ILjava/lang/String;)V");
	assert_eq!(RAW_METHOD.args().len(), 2);
	assert_eq!(RAW_METHOD.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(RAW_METHOD.args()[1], JavaType::Object);
	assert_eq!(RAW_METHOD.ret(), JavaType::Primitive(Primitive::Void));

	assert_eq!(RAW_FIELD.sig().to_bytes(), b"Ljava/lang/String;");
	assert_eq!(RAW_FIELD.ty(), JavaType::Object);

	assert_eq!(RAW_STR, "([I)[[Ljava/lang/Object;");

	assert_eq!(RAW_CSTR.to_bytes(), b"[B");

	assert_eq!(RAW_INNER.sig().to_bytes(), b"(Ljava/lang/Thread$State;)V");
	assert_eq!(RAW_INNER.args()[0], JavaType::Object);
}

// ============================================================================
// Advanced Features
// ============================================================================

fn advanced_features() {
	// String literal with inner class (dollar sign syntax)
	const STRING_INNER: FieldSignature = jni_sig!("java.lang.Thread$State");
	// Default package class (leading dot)
	const DEFAULT_PACKAGE: MethodSignature = jni_sig!((obj: .MyClass) -> void);
	// Type aliases for primitives (convenience)
	const PRIMITIVE_ALIASES: MethodSignature = jni_sig!(
		(a: boolean, b: int, c: long, d: float, e: double) -> void
	);
	// Rust-style primitive aliases
	const RUST_ALIASES: MethodSignature = jni_sig!(
		(a: bool, b: i8, c: i16, d: i32, e: i64, f: f32, g: f64) -> void
	);
	// Void as unit type
	const UNIT_RETURN: MethodSignature = jni_sig!((a: jint) -> ());
	// Custom jni crate path (must be first property)
	const CUSTOM_JNI_PATH: MethodSignature = jni_sig!(
		jni = ::abos_jni,
		(a: jint) -> void
	);
	// Trailing comma allowed
	const TRAILING_COMMA: MethodSignature = jni_sig!(
		type_map = { MyType => com.example.Type },
		(a: MyType) -> void,
	);

	// String literal syntax for Java types (alternative to dot notation)
	const STRING_LITERAL: MethodSignature = jni_sig!(
		(arg: "com.example.Type") -> "com.example.ReturnType"
	);
	assert_eq!(
		STRING_LITERAL.sig().to_bytes(),
		b"(Lcom/example/Type;)Lcom/example/ReturnType;"
	);

	assert_eq!(STRING_INNER.sig().to_bytes(), b"Ljava/lang/Thread$State;");

	assert_eq!(DEFAULT_PACKAGE.sig().to_bytes(), b"(LMyClass;)V");

	assert_eq!(PRIMITIVE_ALIASES.sig().to_bytes(), b"(ZIJFD)V");

	assert_eq!(RUST_ALIASES.sig().to_bytes(), b"(ZBSIJFD)V");

	assert_eq!(UNIT_RETURN.sig().to_bytes(), b"(I)V");

	assert_eq!(CUSTOM_JNI_PATH.sig().to_bytes(), b"(I)V");

	assert_eq!(TRAILING_COMMA.sig().to_bytes(), b"(Lcom/example/Type;)V");
}

fn main() {
	method_signatures();
	field_signatures();
	type_mappings();
	output_variants();
	raw_jni_signatures();
	advanced_features();
}

fn assert_method_signature(
	signature: &MethodSignature,
	descriptor: &[u8],
	arguments: &[JavaType],
	return_type: JavaType,
) {
	assert_eq!(signature.sig().to_bytes(), descriptor);
	assert_eq!(signature.args(), arguments);
	assert_eq!(signature.ret(), return_type);
}
