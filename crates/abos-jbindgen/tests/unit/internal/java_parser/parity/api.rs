use crate::{
	ClassInfo,
	internal::generator::signatures::{generate_dex_signature, generate_field_dex_signature},
};
use std::collections::{BTreeMap, BTreeSet};

const ACC_SYNTHETIC: u16 = 0x1000;
const ACC_BRIDGE: u16 = 0x0040;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MethodFlags {
	pub is_static: bool,
	pub is_native: bool,
	pub is_public: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldFlags {
	pub is_static: bool,
	pub is_final: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeApi {
	pub class_name: String,
	pub parents: BTreeSet<String>,
	pub methods: BTreeMap<String, MethodFlags>,
	pub fields: BTreeMap<String, FieldFlags>,
}

pub fn runtime_api(class: &ClassInfo) -> RuntimeApi {
	let mut api = RuntimeApi {
		class_name: class.class_name.clone(),
		parents: BTreeSet::new(),
		methods: BTreeMap::new(),
		fields: BTreeMap::new(),
	};
	for parent in &class.instance_of {
		assert!(
			api.parents.insert(parent.java_type.replace('.', "/")),
			"duplicate parent in {}",
			class.class_name
		);
	}
	for method in class
		.constructors
		.iter()
		.chain(&class.methods)
		.chain(&class.native_methods)
	{
		let signature: String = generate_dex_signature(&class.class_name, method);
		let flags = MethodFlags {
			is_static: method.is_static,
			is_native: method.is_native,
			is_public: method.is_public,
		};
		assert!(
			api.methods.insert(signature.clone(), flags).is_none(),
			"duplicate method {signature}"
		);
	}
	for field in &class.fields {
		let signature: String = generate_field_dex_signature(&class.class_name, field);
		let flags = FieldFlags {
			is_static: field.is_static,
			is_final: field.is_final,
		};
		assert!(
			api.fields.insert(signature.clone(), flags).is_none(),
			"duplicate field {signature}"
		);
	}
	api
}

pub fn inventory(classes: &[ClassInfo]) -> BTreeMap<String, RuntimeApi> {
	let mut result: BTreeMap<String, RuntimeApi> = BTreeMap::new();
	for class in classes {
		assert!(
			result.insert(class.class_name.clone(), runtime_api(class)).is_none(),
			"duplicate class {}",
			class.class_name
		);
	}
	result
}

/// Match raw compiler artifacts exactly before removing them from the declared API.
/// Keys contain the owner, member name, and complete descriptor, never just a name.
pub fn normalize_bytecode(bytes: &[u8]) -> crate::Result<ClassInfo> {
	let raw: abos_cafebabe::ClassFile = abos_cafebabe::parse_class(bytes)?;
	let mut artifacts: BTreeMap<String, u16> = BTreeMap::new();
	for field in &raw.fields {
		if field.access_flags.0 & ACC_SYNTHETIC != 0 {
			let key: String = format!("L{};->{}:{}", raw.this_class, field.name, field.descriptor);
			assert!(artifacts.insert(key, field.access_flags.0).is_none());
		}
	}
	for method in &raw.methods {
		if method.access_flags.0 & (ACC_SYNTHETIC | ACC_BRIDGE) != 0 {
			let key: String = format!("L{};->{}{}", raw.this_class, method.name, method.descriptor);
			assert!(artifacts.insert(key, method.access_flags.0).is_none());
		}
	}
	assert_artifacts(&raw.this_class, &artifacts);
	let mut class: ClassInfo = crate::internal::bytecode::parse_class(bytes)?;
	let name: &str = &class.class_name;
	class
		.fields
		.retain(|field| !artifacts.contains_key(&generate_field_dex_signature(name, field)));
	class
		.methods
		.retain(|method| !artifacts.contains_key(&generate_dex_signature(name, method)));
	Ok(class)
}

pub fn assert_artifacts(name: &str, actual: &BTreeMap<String, u16>) {
	assert_eq!(
		actual,
		&super::expected::artifacts(name),
		"compiler artifacts for {name}"
	);
}

pub fn assert_api(actual: &BTreeMap<String, RuntimeApi>, expected: &BTreeMap<String, RuntimeApi>, input: &str) {
	assert_keys(actual, expected, input);
	for (name, api) in actual {
		let expected: &RuntimeApi = &expected[name];
		assert_eq!(api.class_name, expected.class_name, "{input}: {name}");
		assert_eq!(api.parents, expected.parents, "{input}: {name} parents");
		assert_members(&api.methods, &expected.methods, input);
		assert_members(&api.fields, &expected.fields, input);
	}
}

fn assert_keys<T>(actual: &BTreeMap<String, T>, expected: &BTreeMap<String, T>, input: &str) {
	let missing: Vec<&String> = expected.keys().filter(|key| !actual.contains_key(*key)).collect();
	let unexpected: Vec<&String> = actual.keys().filter(|key| !expected.contains_key(*key)).collect();
	assert!(
		missing.is_empty() && unexpected.is_empty(),
		"{input}: missing {missing:?}; unexpected {unexpected:?}"
	);
}

fn assert_members<T: std::fmt::Debug + PartialEq>(
	actual: &BTreeMap<String, T>,
	expected: &BTreeMap<String, T>,
	input: &str,
) {
	assert_keys(actual, expected, input);
	for (signature, flags) in actual {
		assert_eq!(flags, &expected[signature], "{input}: {signature}");
	}
}
