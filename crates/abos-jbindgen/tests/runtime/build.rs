//! Compile fixture Java and generate Rust bindings for execution tests.
use std::{env, path::PathBuf};

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
	println!("cargo:rustc-env=FIXTURE_CLASSES={}", classes.display());
	Ok(())
}
