use crate::internal::{bytecode, generator, java_parser};
use crate::{BindgenOptions, Error, ModuleBinding, Result};
use std::{
	fs,
	io::Read,
	path::{Path, PathBuf},
};
/// Check if a class name matches any of the given patterns.
/// Patterns can use wildcards like "android.app.*" or exact matches.
/// For JAR files, patterns match against paths like "com/example/MyClass.class"
pub fn matches_any_pattern(name: &str, patterns: &[String]) -> bool {
	let name: String = name.trim_end_matches(".class").replace('/', ".");
	patterns.iter().any(|pattern| {
		let pattern: String = pattern.replace('/', ".");
		if pattern == "*" || pattern == "**" {
			return true;
		}
		if let Some(prefix) = pattern.strip_suffix(".*") {
			return name == prefix || name.starts_with(&format!("{prefix}."));
		}
		name == pattern || name.starts_with(&format!("{pattern}$"))
	})
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
		classes.push((name.to_owned(), class));
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
	// Parse sources using Java parser, passing all patterns
	let pattern_str: String = if patterns.is_empty() {
		"*".to_string()
	} else {
		patterns.join(",")
	};
	let classes: Vec<crate::ClassInfo> =
		java_parser::parse_java_sources(source_paths, class_path_entries, &pattern_str)?;

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

pub fn visit_jar_classes(
	path: &Path,
	patterns: &[String],
	mut visitor: impl FnMut(&str, &[u8]) -> Result<()>,
) -> Result<()> {
	let mut archive: zip::ZipArchive<fs::File> = zip::ZipArchive::new(fs::File::open(path)?)?;
	for index in 0..archive.len() {
		let mut file: zip::read::ZipFile<'_, fs::File> = archive.by_index(index)?;
		let name: String = file.name().to_owned();
		if Path::new(&name)
			.extension()
			.is_none_or(|extension| extension != "class")
		{
			continue;
		}
		if !patterns.is_empty() && !matches_any_pattern(&name, patterns) {
			continue;
		}
		let mut bytes: Vec<u8> = Vec::new();
		file.read_to_end(&mut bytes)?;
		visitor(&name, &bytes)?;
	}
	Ok(())
}
