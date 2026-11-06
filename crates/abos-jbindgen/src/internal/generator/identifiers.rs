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
#[path = "../../../tests/unit/internal/generator/identifiers.rs"]
mod tests;
