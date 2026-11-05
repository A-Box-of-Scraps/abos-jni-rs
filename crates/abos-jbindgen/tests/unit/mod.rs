use crate::{
	HiddenApiFilter,
	internal::{
		android::hidden::is_api_allowed,
		bytecode::{parse_method_descriptor, parse_type_descriptor},
		inputs::matches_any_pattern,
	},
};

#[test]
fn parses_descriptors_and_rejects_invalid_input() -> crate::Result<()> {
	let ty: crate::TypeInfo = parse_type_descriptor("[[Ljava/lang/String;")?;
	assert_eq!(ty.name, "java.lang.String");
	assert_eq!(ty.array_dimensions, 2);
	let (args, result) = parse_method_descriptor("(I[Ljava/lang/String;)V")?;
	assert_eq!(args.len(), 2);
	assert_eq!(result.name, "void");
	for descriptor in ["", "[", "V", "[V", "L;", "Lmissing", "II"] {
		assert!(parse_type_descriptor(descriptor).is_err(), "{descriptor}");
	}
	for descriptor in ["", "(V)V", "(I", "()", "()VI", "()[V"] {
		assert!(parse_method_descriptor(descriptor).is_err(), "{descriptor}");
	}
	Ok(())
}

#[test]
fn jar_patterns_match_exact_classes_packages_and_inner_classes() {
	for (name, pattern, expected) in [
		("com/example/Foo.class", "com.example.Foo", true),
		("com/example/Foo$Inner.class", "com.example.Foo", true),
		("com/example/FooBar.class", "com.example.Foo", false),
		("other/com/example/Foo.class", "com.example.Foo", false),
		("com/example/Foo.class", "com.example.*", true),
		("any/Foo.class", "**", true),
	] {
		assert_eq!(matches_any_pattern(name, &[pattern.to_owned()]), expected);
	}
}

#[test]
fn hidden_api_flags_respect_explicit_policy() {
	let default: HiddenApiFilter = HiddenApiFilter::default();
	assert!(is_api_allowed(&["public-api"], &default));
	assert!(is_api_allowed(&["sdk"], &default));
	assert!(!is_api_allowed(&["unsupported"], &default));
	assert!(!is_api_allowed(&["blocked"], &default));
	let filter = HiddenApiFilter {
		allow_unsupported: true,
		max_target: Some("p".to_owned()),
	};
	assert!(is_api_allowed(&["unsupported"], &filter));
	assert!(is_api_allowed(&["max-target-q"], &filter));
	assert!(!is_api_allowed(&["max-target-o"], &filter));
}
