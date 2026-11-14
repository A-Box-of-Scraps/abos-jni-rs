use crate::{ClassInfo, HiddenApiFilter, Result};
use std::{
	collections::HashSet,
	fs,
	io::{BufRead, BufReader},
	path::Path,
};
pub fn parse_hiddenapi_flags(path: &Path, filter: &HiddenApiFilter) -> Result<HashSet<String>> {
	let file: std::fs::File = fs::File::open(path)?;
	let reader: std::io::BufReader<std::fs::File> = BufReader::new(file);

	let mut allowed_apis: std::collections::HashSet<String> = HashSet::new();

	for line in reader.lines() {
		let line: String = line?;
		let line: &str = line.trim();

		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		// Format: signature,flag1,flag2,...
		let parts: Vec<&str> = line.split(',').collect();
		if parts.is_empty() {
			continue;
		}

		let signature: &str = parts[0];
		let flags: &[&str] = &parts[1..];

		if is_api_allowed(flags, filter) {
			allowed_apis.insert(signature.to_string());
		}
	}

	Ok(allowed_apis)
}

pub fn is_api_allowed(flags: &[&str], filter: &HiddenApiFilter) -> bool {
	if flags.iter().any(|f| *f == "public-api" || *f == "sdk") {
		return true;
	}

	if filter.allow_unsupported && flags.contains(&"unsupported") {
		return true;
	}

	filter.max_target.as_ref().is_some_and(|maximum| {
		flags
			.iter()
			.filter_map(|flag| flag.strip_prefix("max-target-"))
			.any(|target| compare_target_levels(target, maximum) >= 0)
	})
}

/// Compare Android target levels (e.g., "o" < "p" < "q")
/// Returns: -1 if a < b, 0 if a == b, 1 if a > b
pub fn compare_target_levels(a: &str, b: &str) -> i32 {
	let level_map = [
		("l", 21), // Lollipop
		("m", 23), // Marshmallow
		("n", 24), // Nougat
		("o", 26), // Oreo
		("p", 28), // Pie
		("q", 29), // Android 10
		("r", 30), // Android 11
		("s", 31), // Android 12
		("t", 33), // Android 13
		("u", 34), // Android 14
		("v", 35), // Android 15
	];

	let a_level: Option<i32> = level_map.iter().find(|(k, _)| *k == a).map(|(_, v)| *v);
	let b_level: Option<i32> = level_map.iter().find(|(k, _)| *k == b).map(|(_, v)| *v);

	match (a_level, b_level) {
		(Some(a_val), Some(b_val)) => {
			if a_val < b_val {
				-1
			} else {
				i32::from(a_val > b_val)
			}
		}
		_ => a.cmp(b) as i32,
	}
}

/// Keep only members whose DEX signatures are in the filtered allowlist.
pub fn filter_hidden_apis(classes: Vec<ClassInfo>, hidden_apis: &HashSet<String>) -> Vec<ClassInfo> {
	let mut result: Vec<crate::ClassInfo> = Vec::new();

	for class in classes {
		let filtered: crate::ClassInfo = filter_class_hidden_apis(class, hidden_apis);
		result.push(filtered);
	}

	result
}

pub fn filter_class_hidden_apis(class: ClassInfo, hidden_apis: &HashSet<String>) -> ClassInfo {
	let class_name_dex: String = class.class_name.replace('.', "/");

	let constructors: Vec<crate::MethodInfo> = class
		.constructors
		.into_iter()
		.filter(|m| {
			let sig: String = method_to_dex_signature(&class_name_dex, m);
			hidden_apis.contains(&sig)
		})
		.collect();

	let methods: Vec<crate::MethodInfo> = class
		.methods
		.into_iter()
		.filter(|m| {
			let sig: String = method_to_dex_signature(&class_name_dex, m);
			hidden_apis.contains(&sig)
		})
		.collect();

	let native_methods: Vec<crate::MethodInfo> = class
		.native_methods
		.into_iter()
		.filter(|m| {
			let sig: String = method_to_dex_signature(&class_name_dex, m);
			hidden_apis.contains(&sig)
		})
		.collect();

	let fields: Vec<crate::FieldInfo> = class
		.fields
		.into_iter()
		.filter(|f| {
			let sig: String = field_to_dex_signature(&class_name_dex, f);
			hidden_apis.contains(&sig)
		})
		.collect();

	ClassInfo {
		class_name: class.class_name,
		package: class.package,
		simple_name: class.simple_name,
		documentation: class.documentation,
		rust_name_override: class.rust_name_override,
		constructors,
		methods,
		fields,
		native_methods,
		instance_of: class.instance_of,
	}
}

use crate::internal::generator::signatures::{
	generate_dex_signature as method_to_dex_signature, generate_field_dex_signature as field_to_dex_signature,
};
