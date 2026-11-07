use super::{
	api::{MethodFlags, RuntimeApi, assert_api, inventory, runtime_api},
	expected,
};
use crate::{ClassInfo, FieldInfo, MethodInfo};
use std::collections::BTreeMap;

fn sample() -> ClassInfo {
	ClassInfo {
		class_name: "Sample".to_owned(),
		package: vec![],
		simple_name: "Sample".to_owned(),
		documentation: None,
		rust_name_override: None,
		constructors: vec![],
		fields: vec![],
		methods: vec![],
		native_methods: vec![],
		instance_of: vec![],
	}
}

#[test]
#[should_panic(expected = "duplicate class Sample")]
fn rejects_duplicate_classes() {
	inventory(&[sample(), sample()]);
}

#[test]
#[should_panic(expected = "duplicate method LSample;->echo()I")]
fn rejects_duplicate_methods_even_with_different_flags() {
	let mut class: ClassInfo = sample();
	let method = MethodInfo {
		name: "echo".to_owned(),
		documentation: None,
		rust_name_override: None,
		signature: crate::MethodSignature {
			arguments: vec![],
			return_type: crate::TypeInfo {
				name: "int".to_owned(),
				array_dimensions: 0,
				is_primitive: true,
			},
		},
		is_static: false,
		is_constructor: false,
		is_native: false,
		is_deprecated: false,
		is_public: true,
	};
	class.methods.push(method.clone());
	class.methods.push(MethodInfo {
		is_static: true,
		..method
	});
	runtime_api(&class);
}

#[test]
#[should_panic(expected = "duplicate field LSample;->value:I")]
fn rejects_duplicate_fields() {
	let mut class: ClassInfo = sample();
	let field = FieldInfo {
		name: "value".to_owned(),
		documentation: None,
		rust_name_override: None,
		type_info: crate::TypeInfo {
			name: "int".to_owned(),
			array_dimensions: 0,
			is_primitive: true,
		},
		is_static: false,
		is_final: false,
		is_deprecated: false,
	};
	class.fields = vec![field.clone(), field];
	runtime_api(&class);
}

#[test]
fn rejects_shared_omissions_changed_descriptors_and_flags() {
	let expected: BTreeMap<String, RuntimeApi> = expected::all();
	for mutation in [
		"class",
		"member",
		"descriptor",
		"static",
		"final",
		"native",
		"public",
		"parent",
		"extra",
	] {
		let mut source: BTreeMap<String, RuntimeApi> = expected.clone();
		let api: &mut RuntimeApi = source.get_mut("com/example/ParityMembers").expect("fixture");
		let method: &str = "Lcom/example/ParityMembers;->choose(I)I";
		match mutation {
			"class" => {
				source.remove("I");
			}
			"member" => {
				api.methods.remove(method);
			}
			"descriptor" => {
				let flags: MethodFlags = api.methods.remove(method).expect("method");
				api.methods
					.insert("Lcom/example/ParityMembers;->choose(J)I".to_owned(), flags);
			}
			"static" => api.methods.get_mut(method).expect("method").is_static = true,
			"native" => api.methods.get_mut(method).expect("method").is_native = true,
			"public" => api.methods.get_mut(method).expect("method").is_public = false,
			"final" => {
				api.fields
					.get_mut("Lcom/example/ParityMembers;->value:I")
					.expect("field")
					.is_final = true;
			}
			"parent" => {
				api.parents.insert("WrongParent".to_owned());
			}
			"extra" => {
				api.methods.insert(
					"Lcom/example/ParityMembers;->synthetic()I".to_owned(),
					api.methods[method],
				);
			}
			_ => unreachable!(),
		}
		let bytecode: BTreeMap<String, RuntimeApi> = source.clone();
		assert_eq!(source, bytecode, "differential comparison alone misses {mutation}");
		assert!(
			std::panic::catch_unwind(|| assert_api(&source, &expected, mutation)).is_err(),
			"accepted {mutation}"
		);
	}
}

#[test]
fn rejects_unexpected_missing_and_changed_compiler_artifacts() {
	let name: &str = "com/example/ParityInheritance$Child";
	for mutation in ["unexpected", "missing", "flags"] {
		let mut artifacts: BTreeMap<String, u16> = expected::artifacts(name);
		let key: String = format!("L{name};->number()Ljava/lang/Number;");
		match mutation {
			"unexpected" => {
				artifacts.insert(format!("L{name};->surprise()I"), 0x1041);
			}
			"missing" => {
				artifacts.remove(&key);
			}
			"flags" => {
				artifacts.insert(key, 0x1001);
			}
			_ => unreachable!(),
		}
		assert!(
			std::panic::catch_unwind(|| super::api::assert_artifacts(name, &artifacts)).is_err(),
			"accepted {mutation} artifact"
		);
	}
}
