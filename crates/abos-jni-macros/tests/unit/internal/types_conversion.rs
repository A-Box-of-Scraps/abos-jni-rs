use super::{PrimitiveType, TokenStream, TypeMappings, generate_type_mapping_checks};
use syn::parse::{ParseStream, Parser};

fn checks_for(mappings: &[&str]) -> String {
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mut types: TypeMappings = TypeMappings::new(&jni);
	let input: String = format!("{{{}}}", mappings.join(","));
	(|input: ParseStream<'_>| types.parse_mappings(input))
		.parse_str(&input)
		.expect("valid custom mappings");
	generate_type_mapping_checks(&types, &jni).to_string()
}

#[test]
fn builtin_mappings_generate_no_checks() {
	assert_eq!(checks_for(&[]), "");
}

#[test]
fn mapping_checks_are_independent_of_insertion_order() {
	// Java class names deliberately differ from Rust-path order. References remain
	// before primitives, even when a primitive path sorts before a reference path.
	let sorted_mappings: [&str; 6] = [
		"crate::AlphaRef => example.Zulu",
		"crate::MiddleRef => example.Alpha",
		"crate::ZuluRef => example.Middle",
		"unsafe crate::AlphaPrimitive => long",
		"unsafe crate::MiddlePrimitive => int",
		"unsafe crate::ZuluPrimitive => boolean",
	];
	let expected: String = sorted_mappings
		.iter()
		.map(|mapping| {
			let checks: String = checks_for(&[mapping]);
			assert_ne!(checks, "");
			checks
		})
		.collect::<Vec<String>>()
		.join(" ");

	for rotation in 0..sorted_mappings.len() {
		let mut mappings: [&str; 6] = sorted_mappings;
		mappings.rotate_left(rotation);
		assert_eq!(checks_for(&mappings), expected, "insertion order: {mappings:?}");
		mappings.reverse();
		assert_eq!(checks_for(&mappings), expected, "insertion order: {mappings:?}");
	}
}

#[test]
fn primitive_kind_breaks_ties_for_shared_rust_paths() {
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mut expected: Vec<TokenStream> = Vec::new();
	for primitive in [PrimitiveType::Float, PrimitiveType::Int] {
		let mut types: TypeMappings = TypeMappings::new(&jni);
		types
			.insert_prim_type(primitive.jni_name(), primitive, "crate::Shared", false)
			.expect("custom primitive mapping");
		expected.push(generate_type_mapping_checks(&types, &jni));
	}
	let expected: String = quote::quote!(#(#expected)*).to_string();

	for order in [
		[PrimitiveType::Float, PrimitiveType::Int],
		[PrimitiveType::Int, PrimitiveType::Float],
	] {
		let mut types: TypeMappings = TypeMappings::new(&jni);
		for primitive in order {
			types
				.insert_prim_type(primitive.jni_name(), primitive, "crate::Shared", false)
				.expect("distinct aliases may share a Rust path with different primitive kinds");
		}
		assert_eq!(generate_type_mapping_checks(&types, &jni).to_string(), expected);
	}
}
