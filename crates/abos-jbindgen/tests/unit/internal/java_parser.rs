use super::*;
use crate::internal::generator::signatures::type_to_dex_signature;

#[path = "java_parser/parity/mod.rs"]
mod parity;

fn check_java_descriptors(env: &mut Env<'_>, class: &ClassDescription<'_>) -> abos_jni::errors::Result<()> {
	for methods in [
		class.constructors(env)?,
		class.methods(env)?,
		class.native_methods(env)?,
	] {
		for i in 0..methods.len(env)? {
			let method: MethodDescription<'_> = methods.get_element(env, i)?;
			let info: MethodInfo = java_to_method_info(env, &method)?;
			let result: TypeDescription<'_> = method.return_type(env)?;
			assert_eq!(
				result.descriptor(env)?.to_string(),
				type_to_dex_signature(&info.signature.return_type)
			);
			let parameters: JObjectArray<'_, ParameterDescription<'_>> = method.parameters(env)?;
			for (i, arg) in info.signature.arguments.iter().enumerate() {
				assert_eq!(
					parameters.get_element(env, i)?.descriptor(env)?.to_string(),
					type_to_dex_signature(&arg.type_info)
				);
			}
		}
	}
	let fields: JObjectArray<'_, FieldDescription<'_>> = class.fields(env)?;
	for i in 0..fields.len(env)? {
		let field: FieldDescription<'_> = fields.get_element(env, i)?;
		let info: FieldInfo = java_to_field_info(env, &field)?;
		assert_eq!(
			field.descriptor(env)?.to_string(),
			type_to_dex_signature(&info.type_info)
		);
	}
	Ok(())
}

#[test]
fn rejects_selected_local_classes_with_diagnostics() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/LocalClasses.java");
	for pattern in [
		"*",
		"com.example.LocalClasses$1Captured",
		"com.example.LocalClasses$1Captured$Member",
		"com.example.LocalClasses$1Uncaptured",
	] {
		let error: Error = crate::Builder::new()
			.input_sources(vec![source.clone()], vec![], vec![pattern.to_owned()])
			.generate()
			.expect_err("selected local classes must not generate invalid bindings");
		let message: String = error.to_string();
		assert!(message.contains("Unsupported local class"), "{message}");
		assert!(message.contains("com.example.LocalClasses$1"), "{message}");
		assert!(message.contains("class-file input"), "{message}");
	}
	// Unselected implementation-local classes must not prevent supported member generation.
	let bindings: crate::Bindings = crate::Builder::new()
		.input_sources(
			vec![source],
			vec![],
			vec!["com.example.LocalClasses$Supported".to_owned()],
		)
		.generate()?;
	assert_eq!(bindings.len(), 1);
	Ok(())
}
