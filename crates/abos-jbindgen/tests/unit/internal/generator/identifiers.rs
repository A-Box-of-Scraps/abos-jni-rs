use super::{rust_identifier, unique_identifier, validate_java_member_name};
use std::collections::HashSet;

#[test]
fn normalizes_keywords_characters_and_empty_identifiers() {
	for (input, expected) in [
		("type", "type_"),
		("non_null", "non_null_"),
		("gen", "gen_"),
		("Self", "Self_"),
		("r#match", "match_"),
		("$value", "_value"),
		("with-dash", "with_dash"),
		("λ", "_u3bb_"),
		("1Thing", "_1Thing"),
		("_", "_java"),
		("", "java"),
		("Normal", "Normal"),
	] {
		assert_eq!(rust_identifier(input), expected);
		assert_eq!(rust_identifier(expected), expected);
	}
}

#[test]
fn rejects_java_names_that_would_panic_in_the_downstream_macro() {
	for name in ["type", "gen", "café", "a\u{301}", "_value"] {
		assert!(validate_java_member_name(name).is_ok(), "{name}");
	}
	for name in ["$value", "foo$bar", "¥", "1bad", "", "a-b"] {
		assert!(validate_java_member_name(name).is_err(), "{name}");
	}
}

#[test]
fn allocates_unique_normalized_names() {
	let mut used: HashSet<String> = HashSet::new();
	for (input, expected) in [("type", "type_"), ("type_", "type__"), ("type__", "type___")] {
		assert_eq!(unique_identifier(input, &mut used), expected);
	}
}
