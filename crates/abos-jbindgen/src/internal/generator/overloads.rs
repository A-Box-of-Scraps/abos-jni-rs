use super::names::java_name_to_rust;
use super::signatures::generate_jni_signature_for_sorting;
use crate::MethodInfo;
use std::collections::{HashMap, HashSet};
pub fn group_methods_by_name(methods: &[MethodInfo]) -> Vec<(&str, Vec<&MethodInfo>)> {
	let mut groups: HashMap<&str, Vec<&MethodInfo>> = HashMap::new();
	for method in methods {
		groups.entry(method.name.as_str()).or_default().push(method);
	}

	let mut result: Vec<(&str, Vec<&crate::MethodInfo>)> = groups.into_iter().collect();
	// Sort by name for deterministic output
	result.sort_by_key(|(name, _)| *name);

	for (_, methods) in &mut result {
		methods.sort_by(|a, b| {
			let a_arg_count: usize = a.signature.arguments.len();
			let b_arg_count: usize = b.signature.arguments.len();

			match a_arg_count.cmp(&b_arg_count) {
				std::cmp::Ordering::Equal => {
					let a_sig: String = generate_jni_signature_for_sorting(a);
					let b_sig: String = generate_jni_signature_for_sorting(b);
					a_sig.cmp(&b_sig)
				}
				other => other,
			}
		});
	}

	result
}

/// Reserve a Rust callable name for a full Java member signature or accessor key.
/// Repeated reservations for the same member are stable; other members gain underscores.
/// Returns the name, whether it changed, and the first conflicting member for diagnostics.
pub fn resolve_name_collision(
	rust_name: String,
	member: &str,
	used_names: &mut HashMap<String, String>,
) -> (String, bool, Option<String>) {
	let mut candidate: String = rust_name;
	let mut previous: Option<String> = None;
	while let Some(owner) = used_names.get(&candidate) {
		if owner == member {
			break;
		}
		if previous.is_none() {
			previous = Some(owner.clone());
		}
		candidate.push('_');
	}
	used_names.insert(candidate.clone(), member.to_owned());
	(candidate, previous.is_some(), previous)
}

/// Convert a Java type name to a `snake_case` suffix for method naming
pub fn java_type_to_snake_case_suffix(type_info: &crate::api::types::TypeInfo) -> String {
	let base_name: String = if type_info.is_primitive {
		match type_info.name.as_str() {
			"boolean" => "bool",
			_ => &type_info.name,
		}
		.to_string()
	} else {
		let simple_name: &str = type_info.name.split('.').next_back().unwrap_or(&type_info.name);

		let without_dollar: String = simple_name.replace('$', "");

		java_name_to_rust(&without_dollar).0
	};

	if type_info.array_dimensions > 0 {
		format!("{}_{}d", base_name, type_info.array_dimensions)
	} else {
		base_name
	}
}

/// Generate and reserve one unique name per method, preserving the input order.
///
/// Groups overloads by arity, then adds arity and type suffixes for varying parameter
/// positions. Numeric suffixes resolve remaining collisions with `used_names`.
pub fn generate_unique_overload_names(
	base_name: &str,
	methods: &[&MethodInfo],
	used_names: &mut HashSet<String>,
) -> Vec<String> {
	let mut groups: std::collections::BTreeMap<usize, Vec<(usize, &MethodInfo)>> = std::collections::BTreeMap::new();
	for (index, method) in methods.iter().enumerate() {
		groups
			.entry(method.signature.arguments.len())
			.or_default()
			.push((index, method));
	}
	let mut result: Vec<String> = vec![String::new(); methods.len()];
	for (arity, group) in groups {
		let varying: Vec<usize> = (0..arity)
			.filter(|index| {
				group
					.iter()
					.map(|(_, method)| {
						let ty: &crate::TypeInfo = &method.signature.arguments[*index].type_info;
						(&ty.name, ty.array_dimensions, ty.is_primitive)
					})
					.collect::<HashSet<_>>()
					.len()
					> 1
			})
			.collect();
		for (index, method) in &group {
			let base: String = if group.len() == 1 && !used_names.contains(base_name) {
				base_name.to_owned()
			} else {
				overload_name(base_name, method, arity, &varying)
			};
			let mut candidate: String = base.clone();
			let mut suffix = 1;
			while !used_names.insert(candidate.clone()) {
				candidate = format!("{base}_{suffix}");
				suffix += 1;
			}
			result[*index] = candidate;
		}
	}
	result
}

fn overload_name(base: &str, method: &MethodInfo, arity: usize, varying: &[usize]) -> String {
	let mut name: String = base.to_owned();
	if arity == 1 {
		name.push('_');
		name.push_str(&java_type_to_snake_case_suffix(
			&method.signature.arguments[0].type_info,
		));
		return name;
	}
	if arity >= 2 && varying.len() != arity {
		if base.ends_with(|c: char| c.is_ascii_digit()) {
			name.push_str("_args");
		}
		name.push_str(&arity.to_string());
	}
	for index in varying {
		name.push('_');
		name.push_str(&java_type_to_snake_case_suffix(
			&method.signature.arguments[*index].type_info,
		));
	}
	name
}

pub fn is_snake_case(name: &str) -> bool {
	name.chars()
		.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
#[path = "../../../tests/unit/internal/generator/overloads.rs"]
mod tests;
