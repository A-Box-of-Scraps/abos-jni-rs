use super::*;
use crate::TypeInfo;

#[test]
fn core_classes_never_reserve_generated_names_or_override_builtin_types() -> Result<()> {
	let options = BindgenOptions {
		rust_type_name: Some("SameName".to_owned()),
		..BindgenOptions::default()
	};
	let classes: Vec<ClassInfo> = names::builtin_jni_types()
		.iter()
		.filter(|builtin| builtin.is_core)
		.flat_map(|builtin| [builtin.java_name.to_owned(), builtin.java_name.replace('.', "/")])
		.map(|class_name| ClassInfo {
			class_name,
			package: vec!["java".to_owned(), "lang".to_owned()],
			simple_name: "ignored".to_owned(),
			documentation: None,
			rust_name_override: None,
			constructors: Vec::new(),
			methods: Vec::new(),
			fields: Vec::new(),
			native_methods: Vec::new(),
			instance_of: Vec::new(),
		})
		.collect();
	let map: TypeMap = TypeMap::from_classes(&classes, &options)?;
	assert!(map.is_empty());
	for class in &classes {
		assert!(matches!(
			generate_with_type_map(class, &options, &map),
			Err(Error::Unsupported(_))
		));
		let builtin: &names::BuiltinType = names::core_jni_type(&class.class_name).expect("core class");
		let ty = TypeInfo {
			name: class.class_name.clone(),
			array_dimensions: 1,
			is_primitive: false,
		};
		let mut used: HashSet<String> = HashSet::new();
		assert_eq!(
			names::resolve_type_with_deps(&ty, &map, &mut used),
			format!("{}[]", builtin.rust_name)
		);
		assert!(used.is_empty(), "builtins must not emit custom mappings");
	}
	Ok(())
}
