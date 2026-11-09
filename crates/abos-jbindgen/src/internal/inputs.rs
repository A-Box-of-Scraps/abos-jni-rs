use crate::internal::{bytecode, generator, generator::names::core_jni_type, java_parser};
use crate::{BindgenOptions, Error, ModuleBinding, Result};
use std::{
	fs,
	io::Read,
	path::{Path, PathBuf},
};
/// Match archive names using the class-pattern contract documented on `Builder::patterns`.
/// Uses the same selection rules as the Java parser's `Parser.matchesSinglePattern`.
pub fn matches_any_pattern(name: &str, patterns: &[String]) -> bool {
	let name: String = name.strip_suffix(".class").unwrap_or(name).replace('/', ".");
	patterns.is_empty()
		|| patterns
			.iter()
			.flat_map(|pattern| pattern.split(','))
			.map(str::trim)
			.filter(|pattern| !pattern.is_empty())
			.any(|pattern| matches_class_pattern(&name, &pattern.replace('/', ".")))
}

fn matches_class_pattern(name: &str, pattern: &str) -> bool {
	let name: &[u8] = name.as_bytes();
	let pattern: &[u8] = pattern.as_bytes();
	// matched[i] means the pattern processed so far matches the first i bytes.
	// Only '*' is special; '.' also accepts a binary nesting separator ('$').
	let mut matched: Vec<bool> = vec![false; name.len() + 1];
	matched[0] = true;
	for &token in pattern {
		if token == b'*' {
			for index in 1..=name.len() {
				matched[index] = matched[index] || matched[index - 1];
			}
		} else {
			for index in (1..=name.len()).rev() {
				matched[index] =
					matched[index - 1] && (token == name[index - 1] || (token == b'.' && name[index - 1] == b'$'));
			}
			matched[0] = false;
		}
	}
	matched[name.len()]
		|| (!pattern.contains(&b'*')
			&& name
				.iter()
				.enumerate()
				.any(|(index, &byte)| byte == b'$' && matched[index]))
}

pub fn generate_bindings_with_type_map(
	class_file_path: &Path,
	options: &BindgenOptions,
	extra_type_map: Vec<(String, String)>,
) -> Result<ModuleBinding> {
	let bytes: Vec<u8> = fs::read(class_file_path)?;
	generate_bindings_from_bytes(&bytes, options, extra_type_map)
}

pub fn generate_bindings_from_bytes(
	bytes: &[u8],
	options: &BindgenOptions,
	extra_type_map: Vec<(String, String)>,
) -> Result<ModuleBinding> {
	let class_info: crate::ClassInfo = bytecode::parse_class(bytes)?;

	let mut type_map: crate::TypeMap = crate::TypeMap::from_classes(std::iter::once(&class_info), options)?;
	type_map.merge(extra_type_map);

	generator::generate_with_type_map(&class_info, options, &type_map)
}

pub fn generate_bindings_from_jar(
	jar_path: &Path,
	patterns: &[String],
	options: &BindgenOptions,
	extra_type_map: Vec<(String, String)>,
) -> Result<Vec<ModuleBinding>> {
	let mut classes: Vec<(String, crate::ClassInfo)> = Vec::new();
	visit_jar_classes(jar_path, patterns, |name, bytes| {
		let class: crate::ClassInfo =
			bytecode::parse_class(bytes).map_err(|error| Error::CodeGen(format!("Failed to parse {name}: {error}")))?;
		if core_jni_type(&class.class_name).is_none() {
			classes.push((name.to_owned(), class));
		}
		Ok(())
	})?;

	let mut type_map: crate::TypeMap = crate::TypeMap::from_classes(classes.iter().map(|(_, c)| c), options)?;
	type_map.merge(extra_type_map);

	let mut results: Vec<crate::ModuleBinding> = Vec::new();
	for (name, class_info) in classes {
		let binding: crate::ModuleBinding = generator::generate_with_type_map(&class_info, options, &type_map)
			.map_err(|e| Error::CodeGen(format!("Failed to generate bindings for {name}: {e}")))?;
		results.push(binding);
	}

	Ok(results)
}

pub fn generate_bindings_from_sources(
	source_paths: &[PathBuf],
	class_path_entries: &[PathBuf],
	patterns: &[String],
	options: &BindgenOptions,
	extra_type_map: Vec<(String, String)>,
) -> Result<Vec<ModuleBinding>> {
	validate_class_patterns(patterns)?;
	let pattern_str: String = if patterns.is_empty() {
		"*".to_string()
	} else {
		patterns.join(",")
	};
	let mut classes: Vec<crate::ClassInfo> =
		java_parser::parse_java_sources(source_paths, class_path_entries, &pattern_str)?;
	classes.retain(|class| core_jni_type(&class.class_name).is_none());

	let mut type_map: crate::TypeMap = crate::TypeMap::from_classes(classes.iter(), options)?;
	type_map.merge(extra_type_map);

	let mut results: Vec<crate::ModuleBinding> = Vec::new();

	for class_info in classes {
		let class_name: String = class_info.class_name.clone();

		let binding: crate::ModuleBinding = generator::generate_with_type_map(&class_info, options, &type_map)
			.map_err(|e| Error::CodeGen(format!("Failed to generate bindings for {class_name}: {e}")))?;
		results.push(binding);
	}

	Ok(results)
}

/// Exact core-class requests are errors even when mixed with broader patterns.
pub fn validate_class_patterns(patterns: &[String]) -> Result<()> {
	for pattern in patterns.iter().flat_map(|pattern| pattern.split(',')) {
		generator::validate_class_binding(pattern.trim())?;
	}
	Ok(())
}

pub fn visit_jar_classes(
	path: &Path,
	patterns: &[String],
	mut visitor: impl FnMut(&str, &[u8]) -> Result<()>,
) -> Result<()> {
	validate_class_patterns(patterns)?;
	let mut archive: zip::ZipArchive<fs::File> = zip::ZipArchive::new(fs::File::open(path)?)?;
	// Select from the central-directory names before reading class data. Metadata and
	// versioned entries are never parsed, and host JVM versions do not affect selection.
	let mut names: Vec<String> = Vec::new();
	for name in archive.file_names() {
		let name: std::borrow::Cow<'_, str> = name?;
		if is_binding_class_entry(&name)
			&& core_jni_type(name.trim_end_matches(".class")).is_none()
			&& matches_any_pattern(&name, patterns)
		{
			names.push(name.into_owned());
		}
	}
	names.sort();
	for name in names {
		let mut file: zip::read::ZipFile<'_, fs::File> = archive.by_name(&name)?;
		let mut bytes: Vec<u8> = Vec::new();
		file.read_to_end(&mut bytes)?;
		visitor(&name, &bytes)?;
	}
	Ok(())
}

#[expect(
	clippy::case_sensitive_file_extension_comparisons,
	reason = "JAR class entry names are case-sensitive and use forward slashes on every host."
)]
fn is_binding_class_entry(name: &str) -> bool {
	// JAR paths use '/', independently of the host operating system.
	name.ends_with(".class")
		&& !name.starts_with("META-INF/")
		&& !matches!(
			name.rsplit('/').next(),
			Some("module-info.class" | "package-info.class")
		)
}

#[cfg(test)]
#[path = "../../tests/unit/internal/inputs.rs"]
mod tests;
