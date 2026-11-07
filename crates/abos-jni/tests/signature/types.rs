use super::*;

#[test]
fn test_primitive_types() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: jint, b: jboolean) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(IZ)V");
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Primitive(Primitive::Boolean));
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_java_object_types() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> =
		jni_sig!((a: jint, b: java.lang.String) -> java.lang.Object);

	assert_eq!(sig.sig().to_bytes(), b"(ILjava/lang/String;)Ljava/lang/Object;");
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Object);
}

#[test]
fn test_array_types() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: [jint], b: [java.lang.String]) -> [[jint]]);

	assert_eq!(sig.sig().to_bytes(), b"([I[Ljava/lang/String;)[[I");
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Array);
	assert_eq!(sig.args()[1], JavaType::Array);
	assert_eq!(sig.ret(), JavaType::Array);
}

#[test]
fn test_suffix_array_syntax() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: jint[], b: java.lang.String[][]) -> void);

	assert_eq!(sig.sig().to_bytes(), b"([I[[Ljava/lang/String;)V");
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Array);
	assert_eq!(sig.args()[1], JavaType::Array);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_type_mappings() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		type_map = {
			RustType0 => java.lang.Type0,
			RustType1 => java.lang.Type1,
			RustType2 => java.lang.Type2,
		},
		(a: jint, b: RustType0, c: [RustType1], d: JString) -> RustType2,
	);

	assert_eq!(
		sig.sig().to_bytes(),
		b"(ILjava/lang/Type0;[Ljava/lang/Type1;Ljava/lang/String;)Ljava/lang/Type2;"
	);
	assert_eq!(sig.args().len(), 4);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Object);
	assert_eq!(sig.args()[2], JavaType::Array);
	assert_eq!(sig.args()[3], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Object);
}

#[test]
fn test_trailing_comma_no_type_map() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: jint, b: jboolean) -> void,);

	assert_eq!(sig.sig().to_bytes(), b"(IZ)V");
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Primitive(Primitive::Boolean));
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_trailing_comma_with_type_map() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		(a: RustType) -> void,
		type_map = {
			RustType => java.lang.Type,
		},
	);

	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Type;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_inner_classes() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: java.lang.Outer::Inner) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Outer$Inner;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_default_package() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: .NoPackage) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(LNoPackage;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_primitive_aliases() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: i32, b: bool, c: f64) -> i64);

	assert_eq!(sig.sig().to_bytes(), b"(IZD)J");
	assert_eq!(sig.args().len(), 3);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Primitive(Primitive::Boolean));
	assert_eq!(sig.args()[2], JavaType::Primitive(Primitive::Double));
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Long));
}

#[test]
fn test_builtin_rust_string_type() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: JString) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/String;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_empty_args() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(() -> jint);

	assert_eq!(sig.sig().to_bytes(), b"()I");
	assert_eq!(sig.args().len(), 0);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Int));
}

#[test]
fn test_mixed_builtin_rust_types() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> =
		jni_sig!((a: jint, b: JString, c: [java.lang.Object], d: JThrowable) -> void);

	assert_eq!(
		sig.sig().to_bytes(),
		b"(ILjava/lang/String;[Ljava/lang/Object;Ljava/lang/Throwable;)V"
	);
	assert_eq!(sig.args().len(), 4);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Object);
	assert_eq!(sig.args()[2], JavaType::Array);
	assert_eq!(sig.args()[3], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_field_primitive() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(jint);

	assert_eq!(sig.sig().to_bytes(), b"I");
	assert_eq!(sig.ty(), JavaType::Primitive(Primitive::Int));
}

#[test]
fn test_field_object() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(java.lang.String);

	assert_eq!(sig.sig().to_bytes(), b"Ljava/lang/String;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_field_array() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!([jint]);

	assert_eq!(sig.sig().to_bytes(), b"[I");
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_field_array_suffix() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(java.lang.String[]);

	assert_eq!(sig.sig().to_bytes(), b"[Ljava/lang/String;");
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_field_with_type_mapping() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(
		RustType,
		type_map = {
			RustType => java.lang.Type,
		}
	);

	assert_eq!(sig.sig().to_bytes(), b"Ljava/lang/Type;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_field_2d_array() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!([[jint]]);

	assert_eq!(sig.sig().to_bytes(), b"[[I");
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_field_inner_class() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(java.lang.Outer::Inner);

	assert_eq!(sig.sig().to_bytes(), b"Ljava/lang/Outer$Inner;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_field_default_package() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(.NoPackage);

	assert_eq!(sig.sig().to_bytes(), b"LNoPackage;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_multiple_inner_classes() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((a: java.lang.Outer::Inner::Nested) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Outer$Inner$Nested;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
}

#[test]
fn test_inner_class_field() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(java.lang.Thread::State);

	assert_eq!(sig.sig().to_bytes(), b"Ljava/lang/Thread$State;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_default_package_with_inner_class() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(.OuterClass::InnerClass);

	assert_eq!(sig.sig().to_bytes(), b"LOuterClass$InnerClass;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_array_of_inner_classes() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!([java.lang.Thread::State]);

	assert_eq!(sig.sig().to_bytes(), b"[Ljava/lang/Thread$State;");
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_inner_class_array_suffix() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(java.lang.Outer::Inner[][]);

	assert_eq!(sig.sig().to_bytes(), b"[[Ljava/lang/Outer$Inner;");
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_deeply_nested_inner_classes() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(com.example.Outer::Middle::Inner::Deep);

	assert_eq!(sig.sig().to_bytes(), b"Lcom/example/Outer$Middle$Inner$Deep;");
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_default_package_class_explicit() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!((arg: .DefaultPackageClass) -> void);

	assert_eq!(sig.sig().to_bytes(), b"(LDefaultPackageClass;)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
}

#[test]
fn test_default_package_nested_inner_classes() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(.Outer::Inner1::Inner2);

	assert_eq!(sig.sig().to_bytes(), b"LOuter$Inner1$Inner2;");
	assert_eq!(sig.ty(), JavaType::Object);
}
