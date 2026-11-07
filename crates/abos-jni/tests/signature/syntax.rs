use super::*;

#[test]
fn test_raw_method_signature_str() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!("(ILjava/lang/String;)V");
	assert_eq!(sig.sig().as_ptr(), c"(ILjava/lang/String;)V".as_ptr());
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Primitive(Primitive::Int));
	assert_eq!(sig.args()[1], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_raw_method_signature_cstr() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(c"(Ljava/lang/String;)I");
	assert_eq!(sig.sig().as_ptr(), c"(Ljava/lang/String;)I".as_ptr());
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.args()[0], JavaType::Object);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Int));
}

#[test]
fn test_raw_field_signature_primitive() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("I");
	assert_eq!(sig.sig().as_ptr(), c"I".as_ptr());
	assert_eq!(sig.ty(), JavaType::Primitive(Primitive::Int));
}

#[test]
fn test_raw_field_signature_object() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("Ljava/lang/String;");
	assert_eq!(sig.sig().as_ptr(), c"Ljava/lang/String;".as_ptr());
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_raw_field_signature_array() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("[I");
	assert_eq!(sig.sig().as_ptr(), c"[I".as_ptr());
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_raw_field_signature_multidim_array() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("[[Ljava/lang/String;");
	assert_eq!(sig.sig().as_ptr(), c"[[Ljava/lang/String;".as_ptr());
	assert_eq!(sig.ty(), JavaType::Array);
}

#[test]
fn test_raw_method_with_arrays() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!("([I[Ljava/lang/String;)[[I");
	assert_eq!(sig.sig().as_ptr(), c"([I[Ljava/lang/String;)[[I".as_ptr());
	assert_eq!(sig.args().len(), 2);
	assert_eq!(sig.args()[0], JavaType::Array);
	assert_eq!(sig.args()[1], JavaType::Array);
	assert_eq!(sig.ret(), JavaType::Array);
}

#[test]
fn test_raw_method_empty_args() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!("()V");
	assert_eq!(sig.sig().as_ptr(), c"()V".as_ptr());
	assert_eq!(sig.args().len(), 0);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_raw_field_with_inner_class() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("Ljava/lang/Outer$Inner;");
	assert_eq!(sig.sig().as_ptr(), c"Ljava/lang/Outer$Inner;".as_ptr());
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_raw_field_default_package() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!("LNoPackage;");
	assert_eq!(sig.sig().as_ptr(), c"LNoPackage;".as_ptr());
	assert_eq!(sig.ty(), JavaType::Object);
}

#[test]
fn test_named_sig_property() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(sig = (a: jint) -> void);
	assert_eq!(sig.sig().to_bytes(), b"(I)V");
	assert_eq!(sig.args().len(), 1);
	assert_eq!(sig.ret(), JavaType::Primitive(Primitive::Void));
}

#[test]
fn test_named_sig_property_field() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(sig = jint);
	assert_eq!(sig.sig().to_bytes(), b"I");
	assert_eq!(sig.ty(), JavaType::Primitive(Primitive::Int));
}

#[test]
fn test_type_map_with_named_sig() {
	let sig1: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		sig = (a: RustType) -> void,
		type_map = {
			RustType => java.lang.Type,
		}
	);

	let sig2: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		type_map = {
			RustType => java.lang.Type,
		},
		sig = (a: RustType) -> void
	);

	assert_eq!(sig1.sig().to_bytes(), b"(Ljava/lang/Type;)V");
	assert_eq!(sig2.sig().to_bytes(), b"(Ljava/lang/Type;)V");
}

#[test]
fn test_jni_crate_override() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		(a: jint) -> void
	);
	assert_eq!(sig.sig().to_bytes(), b"(I)V");
}

#[test]
fn test_all_properties_together() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		type_map = {
			CustomType => java.lang.Custom,
		},
		sig = (a: CustomType) -> void,
	);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_unnamed_sig_with_other_props() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		(a: CustomType) -> void,
		type_map = {
			CustomType => java.lang.Custom,
		}
	);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_unnamed_sig_in_middle() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		type_map = {
			CustomType => java.lang.Custom,
		},
		(a: CustomType) -> void
	);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_unnamed_sig_at_end() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		type_map = {
			CustomType => java.lang.Custom,
		},
		(a: CustomType) -> void
	);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_unnamed_sig_with_trailing_comma() {
	let sig: abos_jni::signature::MethodSignature<'_, '_> = jni_sig!(
		jni = ::abos_jni,
		type_map = {
			CustomType => java.lang.Custom,
		},
		(a: CustomType) -> void,
	);
	assert_eq!(sig.sig().to_bytes(), b"(Ljava/lang/Custom;)V");
}

#[test]
fn test_type_map_prefix_required() {
	let sig: abos_jni::signature::FieldSignature<'_> = jni_sig!(
		CustomType,
		type_map = {
			CustomType => java.lang.Custom,
		}
	);
	assert_eq!(sig.sig().to_bytes(), b"Ljava/lang/Custom;");
}
