use std::{collections::HashSet, fmt::Write as _};

/// Produce an ASCII Rust identifier, also suitable for a module directory name.
/// Keep Java names separately: this spelling is only for the generated Rust API.
pub fn rust_identifier(name: &str) -> String {
	let mut identifier = String::new();
	for character in name.strip_prefix("r#").unwrap_or(name).chars() {
		if character.is_ascii_alphanumeric() || character == '_' {
			identifier.push(character);
		} else if character.is_ascii() {
			identifier.push('_');
		} else {
			// Encoding non-ASCII characters avoids filesystem and Unicode-normalization
			// differences between inline modules, file trees, and Rust identifiers.
			write!(identifier, "_u{:x}_", u32::from(character)).expect("writing to String");
		}
	}
	if identifier.starts_with(|character: char| character.is_ascii_digit()) {
		identifier.insert(0, '_');
	}
	if identifier.is_empty() || identifier == "_" {
		identifier.push_str("java");
	}
	// non_null is parsed as a field qualifier by bind_java_type!, not as a name.
	if is_keyword(&identifier) || identifier == "non_null" {
		identifier.push('_');
	}
	identifier
}

/// The downstream macro converts explicit Java member names into Rust identifiers,
/// even though it discards that identifier. Reject names that would make it panic.
/// Temporary workaround: see JNI-MACROS-001 in docs/ISSUES.md for the fork fix and removal checklist.
pub fn validate_java_member_name(name: &str) -> crate::Result<()> {
	let mut characters: std::str::Chars<'_> = name.chars();
	if characters
		.next()
		.is_some_and(|first| first == '_' || unicode_ident::is_xid_start(first))
		&& characters.all(unicode_ident::is_xid_continue)
	{
		return Ok(());
	}
	Err(crate::Error::Unsupported(format!(
		"abos_jni::bind_java_type! cannot represent Java member name {name:?}; omit it with skip_signature"
	)))
}

fn is_keyword(name: &str) -> bool {
	matches!(
		name,
		"as" | "break"
			| "const" | "continue"
			| "crate" | "else"
			| "enum" | "extern"
			| "false" | "fn"
			| "for" | "if"
			| "impl" | "in"
			| "let" | "loop"
			| "match" | "mod"
			| "move" | "mut"
			| "pub" | "ref"
			| "return"
			| "self" | "Self"
			| "static"
			| "struct"
			| "super" | "trait"
			| "true" | "type"
			| "unsafe"
			| "use" | "where"
			| "while" | "async"
			| "await" | "dyn"
			| "abstract"
			| "become"
			| "box" | "do"
			| "final" | "gen"
			| "macro" | "override"
			| "priv" | "typeof"
			| "unsized"
			| "virtual"
			| "yield" | "try"
	)
}

/// Reserve a name after normalization so distinct Java spellings remain distinct.
pub fn unique_identifier(name: &str, used: &mut HashSet<String>) -> String {
	let mut identifier: String = rust_identifier(name);
	while !used.insert(identifier.clone()) {
		identifier.push('_');
	}
	identifier
}

#[cfg(test)]
mod tests {
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
}
