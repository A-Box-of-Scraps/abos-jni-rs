//! Source parsing can reuse a JVM initialized by the embedding application.
use abos_jbindgen::{Bindings, Builder};
use jni::{InitArgs, InitArgsBuilder, JNIVersion, JavaVM};
use std::path::PathBuf;

#[test]
fn reuses_an_existing_jvm() -> Result<(), Box<dyn std::error::Error>> {
	let args: InitArgs<'_> = InitArgsBuilder::new().version(JNIVersion::V1_8).build()?;
	let _vm: JavaVM = JavaVM::new(args)?;
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/SimpleClass.java");
	let bindings: Bindings = Builder::new()
		.input_sources(vec![source], Vec::new(), Vec::new())
		.generate()?;
	assert_eq!(bindings.len(), 1);
	Ok(())
}
