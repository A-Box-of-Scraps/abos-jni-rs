use super::*;
use assert_matches::assert_matches;

#[test]
fn test_parser_types() {
	let cases: [(&str, JavaType); 18] = [
		("Z", JavaType::Primitive(Primitive::Boolean)),
		("B", JavaType::Primitive(Primitive::Byte)),
		("C", JavaType::Primitive(Primitive::Char)),
		("S", JavaType::Primitive(Primitive::Short)),
		("I", JavaType::Primitive(Primitive::Int)),
		("J", JavaType::Primitive(Primitive::Long)),
		("F", JavaType::Primitive(Primitive::Float)),
		("D", JavaType::Primitive(Primitive::Double)),
		("V", JavaType::Primitive(Primitive::Void)),
		("Ljava/lang/String;", JavaType::Object),
		("[I", JavaType::Array),
		("[Ljava/lang/String;", JavaType::Array),
		("[[I", JavaType::Array),
		("[[[I", JavaType::Array),
		("[[Ljava/lang/String;", JavaType::Array),
		("LString;", JavaType::Object),
		("LA;", JavaType::Object),
		("La/b/c/d/e;", JavaType::Object),
	];
	for (input, expected) in cases {
		assert_eq!(
			input.parse::<JavaType>().expect("valid type descriptor"),
			expected,
			"{input}"
		);
	}
	// Reject incomplete descriptors, trailing input, invalid class names and void arrays.
	for input in [
		"",
		"A",
		"Invalid",
		"II",
		"java/lang/String",
		"Ljava/lang/String",
		"java/lang/String;",
		"L/java/lang/String;",
		"L/;",
		"L;",
		"[V",
		"[",
		"[[",
		"Ljava/lang/String/;",
		"LString/;",
		"Ljava//lang;",
		"Ljava.lang.String;",
		"Ljava[lang;",
	] {
		assert!(input.parse::<JavaType>().is_err(), "{input}");
	}
	for input in ["V", "[V"] {
		assert!(input.parse::<RuntimeFieldSignature>().is_err(), "{input}");
	}
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
	for &(input, args, ret) in METHOD_CASES {
		assert_eq!(
			input
				.parse::<RuntimeMethodSignature>()
				.expect("valid method descriptor"),
			RuntimeMethodSignature {
				sig: JNIString::new(input),
				args: args.to_vec(),
				ret
			},
			"{input}"
		);
	}
	// Reject incomplete signatures, nested method arguments, invalid names and void arguments/arrays.
	for input in [
		"",
		"()",
		"V",
		"(I",
		"I)I",
		"(I)",
		"(Invalid)I",
		"((()I)I)I",
		"(I)V ",
		"()java/lang/List",
		"(L/java/lang/String)V",
		"([V)V",
		"(I[V)V",
		"([VI)V",
		"()[V",
		"(I)[V",
		"(V)I",
		"(V)V",
		"(IV)I",
		"(VI)I",
	] {
		assert!(input.parse::<RuntimeMethodSignature>().is_err(), "{input}");
	}
}

const METHOD_CASES: &[(&str, &[JavaType], ReturnType)] = &[
	("()V", &[], ReturnType::Primitive(Primitive::Void)),
	(
		"(I)V",
		&[JavaType::Primitive(Primitive::Int)],
		ReturnType::Primitive(Primitive::Void),
	),
	(
		"(Ljava/lang/String;)I",
		&[JavaType::Object],
		ReturnType::Primitive(Primitive::Int),
	),
	("([I)I", &[JavaType::Array], ReturnType::Primitive(Primitive::Int)),
	(
		"([Ljava/lang/String;)I",
		&[JavaType::Array],
		ReturnType::Primitive(Primitive::Int),
	),
	(
		"(I[Ljava/lang/String;Z)I",
		&[
			JavaType::Primitive(Primitive::Int),
			JavaType::Array,
			JavaType::Primitive(Primitive::Boolean),
		],
		ReturnType::Primitive(Primitive::Int),
	),
	("()Ljava/lang/String;", &[], ReturnType::Object),
	("()[I", &[], ReturnType::Array),
	("()[Ljava/lang/String;", &[], ReturnType::Array),
	(
		"(Ljava/lang/String;Ljava/lang/Object;)V",
		&[JavaType::Object, JavaType::Object],
		ReturnType::Primitive(Primitive::Void),
	),
	(
		"([[I[Ljava/lang/String;)V",
		&[JavaType::Array, JavaType::Array],
		ReturnType::Primitive(Primitive::Void),
	),
];

#[test]
fn deeply_nested_arrays_do_not_grow_the_stack() {
	for (element, valid) in [("I", true), ("Ljava/lang/String;", true), ("V", false), ("", false)] {
		let descriptor: String = format!("{}{element}", "[".repeat(65_536));
		assert_eq!(descriptor.parse::<JavaType>().is_ok(), valid);
		assert_eq!(RuntimeFieldSignature::from_str(&descriptor).is_ok(), valid);
		assert_eq!(
			RuntimeMethodSignature::from_str(format!("({descriptor})V")).is_ok(),
			valid
		);
		assert_eq!(
			RuntimeMethodSignature::from_str(format!("(){descriptor}")).is_ok(),
			valid
		);
	}
}
