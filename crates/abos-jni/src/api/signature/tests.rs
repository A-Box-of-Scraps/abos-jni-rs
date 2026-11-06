use super::*;
use assert_matches::assert_matches;

#[test]
fn test_parser_types() {
	assert_eq!(
		"Z".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Boolean)
	);
	assert_eq!(
		"B".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Byte)
	);
	assert_eq!(
		"C".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Char)
	);
	assert_eq!(
		"S".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Short)
	);
	assert_eq!(
		"I".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Int)
	);
	assert_eq!(
		"J".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Long)
	);
	assert_eq!(
		"F".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Float)
	);
	assert_eq!(
		"D".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Double)
	);
	assert_eq!(
		"V".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Primitive(Primitive::Void)
	);
	assert_eq!(
		"Ljava/lang/String;"
			.parse::<JavaType>()
			.expect("test operation succeeds"),
		JavaType::Object
	);
	assert_eq!(
		"[I".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Array
	);
	assert_eq!(
		"[Ljava/lang/String;"
			.parse::<JavaType>()
			.expect("test operation succeeds"),
		JavaType::Array
	);

	assert_matches!("".parse::<JavaType>(), Err(_));
	assert_matches!("A".parse::<JavaType>(), Err(_));
	// The parser should return an error if the entire input is not consumed (#598)
	assert_matches!("Invalid".parse::<JavaType>(), Err(_));
	assert_matches!("II".parse::<JavaType>(), Err(_));
	assert_matches!("java/lang/String".parse::<JavaType>(), Err(_));
	assert_matches!("Ljava/lang/String".parse::<JavaType>(), Err(_));
	assert_matches!("java/lang/String;".parse::<JavaType>(), Err(_));
	// Don't allow leading '/' in class names (#212)
	assert_matches!("L/java/lang/String;".parse::<JavaType>(), Err(_));
	assert_matches!("L/;".parse::<JavaType>(), Err(_));
	assert_matches!("L;".parse::<JavaType>(), Err(_));

	// Void field types are invalid
	assert_matches!("V".parse::<RuntimeFieldSignature>(), Err(_));

	// Void arrays are invalid
	assert_matches!("[V".parse::<JavaType>(), Err(_));
	assert_matches!("[V".parse::<RuntimeFieldSignature>(), Err(_));

	// Multi-dimensional arrays
	assert_eq!(
		"[[I".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Array
	);
	assert_eq!(
		"[[[I".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Array
	);
	assert_eq!(
		"[[Ljava/lang/String;"
			.parse::<JavaType>()
			.expect("test operation succeeds"),
		JavaType::Array
	);

	// Array with no element type
	assert_matches!("[".parse::<JavaType>(), Err(_));
	assert_matches!("[[".parse::<JavaType>(), Err(_));

	// Single-segment class name (no slashes)
	assert_eq!(
		"LString;".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Object
	);
	assert_eq!(
		"LA;".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Object
	);

	// Deeply nested class path
	assert_eq!(
		"La/b/c/d/e;".parse::<JavaType>().expect("test operation succeeds"),
		JavaType::Object
	);

	// Trailing slash in class name
	assert_matches!("Ljava/lang/String/;".parse::<JavaType>(), Err(_));
	assert_matches!("LString/;".parse::<JavaType>(), Err(_));

	// Double slash in class name
	assert_matches!("Ljava//lang;".parse::<JavaType>(), Err(_));

	// Dots in class name (Java-style, not JVM-style)
	assert_matches!("Ljava.lang.String;".parse::<JavaType>(), Err(_));

	// '[' in class name
	assert_matches!("Ljava[lang;".parse::<JavaType>(), Err(_));
}

#[test]
fn test_parser_field_signatures() {
	// Primitive field signatures
	assert_eq!(
		"I".parse::<RuntimeFieldSignature>().expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("I"),
			ty: JavaType::Primitive(Primitive::Int),
		}
	);
	assert_eq!(
		"Z".parse::<RuntimeFieldSignature>().expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("Z"),
			ty: JavaType::Primitive(Primitive::Boolean),
		}
	);

	// Object field signature
	assert_eq!(
		"Ljava/lang/String;"
			.parse::<RuntimeFieldSignature>()
			.expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("Ljava/lang/String;"),
			ty: JavaType::Object,
		}
	);

	// Array field signatures
	assert_eq!(
		"[I".parse::<RuntimeFieldSignature>().expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("[I"),
			ty: JavaType::Array,
		}
	);
	assert_eq!(
		"[Ljava/lang/String;"
			.parse::<RuntimeFieldSignature>()
			.expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("[Ljava/lang/String;"),
			ty: JavaType::Array,
		}
	);
	assert_eq!(
		"[[I".parse::<RuntimeFieldSignature>().expect("test operation succeeds"),
		RuntimeFieldSignature {
			sig: JNIString::new("[[I"),
			ty: JavaType::Array,
		}
	);

	// Invalid field signatures
	assert_matches!("".parse::<RuntimeFieldSignature>(), Err(_));
	assert_matches!("V".parse::<RuntimeFieldSignature>(), Err(_));
	assert_matches!("[V".parse::<RuntimeFieldSignature>(), Err(_));
	assert_matches!("IZ".parse::<RuntimeFieldSignature>(), Err(_));
	assert_matches!("Ljava/lang/String;I".parse::<RuntimeFieldSignature>(), Err(_));
	assert_matches!("X".parse::<RuntimeFieldSignature>(), Err(_));
}

#[test]
fn test_parser_signatures() {
	assert_eq!(
		"()V"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("()V"),
			args: vec![],
			ret: ReturnType::Primitive(Primitive::Void)
		}
	);
	assert_eq!(
		"(I)V"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("(I)V"),
			args: vec![JavaType::Primitive(Primitive::Int)],
			ret: ReturnType::Primitive(Primitive::Void)
		}
	);
	assert_eq!(
		"(Ljava/lang/String;)I"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("(Ljava/lang/String;)I"),
			args: vec![JavaType::Object],
			ret: ReturnType::Primitive(Primitive::Int)
		}
	);
	assert_eq!(
		"([I)I"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("([I)I"),
			args: vec![JavaType::Array],
			ret: ReturnType::Primitive(Primitive::Int)
		}
	);
	assert_eq!(
		"([Ljava/lang/String;)I"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("([Ljava/lang/String;)I"),
			args: vec![JavaType::Array],
			ret: ReturnType::Primitive(Primitive::Int)
		}
	);
	assert_eq!(
		"(I[Ljava/lang/String;Z)I"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("(I[Ljava/lang/String;Z)I"),
			args: vec![
				JavaType::Primitive(Primitive::Int),
				JavaType::Array,
				JavaType::Primitive(Primitive::Boolean),
			],
			ret: ReturnType::Primitive(Primitive::Int)
		}
	);

	assert_matches!("".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("()".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("V".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(I".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("I)I".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(I)".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(Invalid)I".parse::<RuntimeMethodSignature>(), Err(_));
	// We shouldn't recursively allow method signatures as method argument types (#597)
	assert_matches!("((()I)I)I".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(I)V ".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("()java/lang/List".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(L/java/lang/String)V".parse::<RuntimeMethodSignature>(), Err(_));

	// Void arrays are invalid in method arguments
	assert_matches!("([V)V".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(I[V)V".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("([VI)V".parse::<RuntimeMethodSignature>(), Err(_));
	// Void arrays are invalid as return types
	assert_matches!("()[V".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(I)[V".parse::<RuntimeMethodSignature>(), Err(_));

	// Void as method argument is invalid
	assert_matches!("(V)I".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(V)V".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(IV)I".parse::<RuntimeMethodSignature>(), Err(_));
	assert_matches!("(VI)I".parse::<RuntimeMethodSignature>(), Err(_));

	// Object return type
	assert_eq!(
		"()Ljava/lang/String;"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("()Ljava/lang/String;"),
			args: vec![],
			ret: ReturnType::Object
		}
	);

	// Array return type
	assert_eq!(
		"()[I"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("()[I"),
			args: vec![],
			ret: ReturnType::Array
		}
	);
	assert_eq!(
		"()[Ljava/lang/String;"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("()[Ljava/lang/String;"),
			args: vec![],
			ret: ReturnType::Array
		}
	);

	// Multiple object arguments
	assert_eq!(
		"(Ljava/lang/String;Ljava/lang/Object;)V"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("(Ljava/lang/String;Ljava/lang/Object;)V"),
			args: vec![JavaType::Object, JavaType::Object],
			ret: ReturnType::Primitive(Primitive::Void)
		}
	);

	// Array arguments
	assert_eq!(
		"([[I[Ljava/lang/String;)V"
			.parse::<RuntimeMethodSignature>()
			.expect("test operation succeeds"),
		RuntimeMethodSignature {
			sig: JNIString::new("([[I[Ljava/lang/String;)V"),
			args: vec![JavaType::Array, JavaType::Array],
			ret: ReturnType::Primitive(Primitive::Void)
		}
	);
}
