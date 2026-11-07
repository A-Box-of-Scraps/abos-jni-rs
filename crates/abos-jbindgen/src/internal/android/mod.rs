pub mod sdk;
use crate::internal::{bytecode, generator, java_parser};
use crate::{AndroidSdk, BindgenOptions, ClassInfo, Error, HiddenApiFilter, ModuleBinding, Result, TypeMap};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};
pub mod hidden;
pub mod intersection;
use hidden::{filter_hidden_apis, parse_hiddenapi_flags};
use intersection::intersect_classes;
pub fn generate_android_bindings(
	api_level: u32,
	patterns: &[String],
	hiddenapi_flags_path: Option<&Path>,
	hiddenapi_filter: &HiddenApiFilter,
	options: &BindgenOptions,
	extra_type_map: Vec<(String, String)>,
) -> Result<Vec<ModuleBinding>> {
	let sdk: crate::AndroidSdk = AndroidSdk::from_env(api_level)?;

	// Parse android.jar (bytecode) for actual implementation
	let android_jar: std::path::PathBuf = sdk.get_android_jar()?;
	let jar_classes: std::collections::HashMap<String, crate::ClassInfo> = parse_jar_classes(&android_jar, patterns)?;

	// Parse android-stubs-src.jar (source stubs) for public API surface
	let stubs_jar: std::path::PathBuf = sdk.get_stubs_src_jar()?;
	let classpath: Vec<std::path::PathBuf> = sdk.get_classpath()?;
	let stub_classes: std::collections::HashMap<String, crate::ClassInfo> =
		parse_source_stubs(&stubs_jar, &classpath, patterns)?;

	let mut filtered_classes: Vec<crate::ClassInfo> = intersect_classes(jar_classes, &stub_classes);

	if let Some(flags_path) = hiddenapi_flags_path {
		let hidden_apis: std::collections::HashSet<String> = parse_hiddenapi_flags(flags_path, hiddenapi_filter)?;
		filtered_classes = filter_hidden_apis(filtered_classes, &hidden_apis);
	}

	filtered_classes.sort_by(|left, right| left.class_name.cmp(&right.class_name));

	let mut type_map: crate::TypeMap = TypeMap::from_classes(filtered_classes.iter(), options)?;
	type_map.merge(extra_type_map);

	let mut results: Vec<crate::ModuleBinding> = Vec::new();
	for class_info in filtered_classes {
		let binding: ModuleBinding =
			generator::generate_with_type_map(&class_info, options, &type_map).map_err(|source| {
				Error::AndroidClassGeneration {
					class_name: class_info.class_name.replace('/', "."),
					source: Box::new(source),
				}
			})?;
		results.push(binding);
	}

	Ok(results)
}

/// Parse class files from a JAR archive
pub(super) fn parse_jar_classes(jar_path: &Path, patterns: &[String]) -> Result<HashMap<String, ClassInfo>> {
	let mut classes: HashMap<String, ClassInfo> = HashMap::new();
	crate::internal::inputs::visit_jar_classes(jar_path, patterns, |name, bytes| {
		let class: ClassInfo = bytecode::parse_class(bytes).map_err(|source| Error::AndroidClassParse {
			archive: jar_path.to_owned(),
			entry: name.to_owned(),
			source: Box::new(source),
		})?;
		classes.insert(class.class_name.replace('/', "."), class);
		Ok(())
	})?;

	Ok(classes)
}

/// Parse source stubs from android-stubs-src.jar
pub(super) fn parse_source_stubs(
	stubs_jar: &Path,
	classpath: &[PathBuf],
	patterns: &[String],
) -> Result<HashMap<String, ClassInfo>> {
	let pattern_str: String = if patterns.is_empty() {
		"*".to_string()
	} else {
		patterns.join(",")
	};

	let classes: Vec<crate::ClassInfo> =
		java_parser::parse_java_sources(&[stubs_jar.to_path_buf()], classpath, &pattern_str)?;

	let mut result: std::collections::HashMap<String, crate::ClassInfo> = HashMap::new();
	for class_info in classes {
		let key: String = class_info.class_name.replace('/', ".");
		result.insert(key, class_info);
	}

	Ok(result)
}
