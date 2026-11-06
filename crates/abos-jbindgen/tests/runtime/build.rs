//! Compile fixture Java and generate Rust bindings for execution tests.
#[path = "../support/jars.rs"]
mod jar_fixtures;
use std::{
	env,
	path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR missing")?);
	let classes = output.join("classes");
	let names = [
		"SimpleClass",
		"Calculator",
		"NoPackage",
		"OuterClass",
		"Overloads",
		"AwkwardNames",
		"NameCollisionTest",
		"WithFields",
		"NativeMethodVisibility",
		"WithNative",
		"AccessorCollisions",
		"MemberCollisions",
		"MixedMethods",
		"Identifiers",
		"UnsupportedNames",
		"keywords/match",
	];
	let sources: Vec<PathBuf> = names
		.iter()
		.map(|name| PathBuf::from("../java").join(format!("{name}.java")))
		.collect();
	abos_javac::Build::new()
		.files(&sources)
		.output_dir(&classes)
		.emit_rerun_if_changed(true)
		.try_compile()?;
	let bindings = abos_jbindgen::Builder::new()
		.input_sources(sources, Vec::new(), Vec::new())
		.root_path("crate::bindings")
		.generate_native_interfaces(false)
		.generate()?;
	bindings.write_to_file(output.join("bindings.rs"))?;
	bindings.write_to_files(output.join("tree"))?;
	abos_jbindgen::Builder::new()
		.input_class(classes.join("com/type/Identifiers.class"))
		.root_path("crate::bytecode")
		.type_mapping("crate::bindings::com::type_::match_", "com.type.match")
		.generate()?
		.write_to_file(output.join("bytecode.rs"))?;
	abos_jbindgen::Builder::new()
		.input_class(classes.join("com/example/SimpleClass.class"))
		.root_path("crate::overrides")
		.rust_type_name("type")
		.name_override("Lcom/example/SimpleClass;-><init>()V", "gen")
		.name_override("Lcom/example/SimpleClass;->getValue()I", "with-dash")
		.generate()?
		.write_to_file(output.join("overrides.rs"))?;
	abos_jbindgen::Builder::new()
		.input_class(classes.join("com/example/MemberCollisions.class"))
		.root_path("crate::collision_overrides")
		.name_override("Lcom/example/MemberCollisions;-><init>(I)V", "make")
		.name_override("Lcom/example/MemberCollisions;-><init>(Ljava/lang/String;)V", "make")
		.name_override("Lcom/example/MemberCollisions;->choose(I)I", "pick-name")
		.name_override(
			"Lcom/example/MemberCollisions;->choose(Ljava/lang/String;)I",
			"pick_name",
		)
		.generate()?
		.write_to_file(output.join("collision_overrides.rs"))?;
	abos_jbindgen::Builder::new()
		.input_class(classes.join("com/example/MixedMethods.class"))
		.root_path("crate::native_collisions")
		.generate()?
		.write_to_file(output.join("native_collisions.rs"))?;
	archive_bindings(&output)?;
	Ok(())
}

fn archive_bindings(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let sources = Path::new("../java/archives");
	println!("cargo:rerun-if-changed={}", sources.display());
	let jars = jar_fixtures::create(sources, &output.join("jars"))?;
	for (name, path) in [
		("modular_jar", &jars.modular),
		("multi_release_jar", &jars.multi_release),
		("reversed_jar", &jars.reversed),
	] {
		abos_jbindgen::Builder::new()
			.input_jar(path)
			.root_path(format!("crate::{name}"))
			.generate()?
			.write_to_file(output.join(format!("{name}.rs")))?;
	}
	let classpath = env::join_paths([output.join("classes"), jars.modular, jars.multi_release])?;
	println!(
		"cargo:rustc-env=FIXTURE_CLASSES={}",
		classpath.to_str().ok_or("non-UTF-8 fixture classpath")?
	);
	Ok(())
}
