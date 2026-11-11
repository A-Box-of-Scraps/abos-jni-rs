#[path = "../support/generate.rs"]
mod generate;
#[path = "../support/types.rs"]
mod types;

use std::{fs, path::Path, process::Command};

fn check_alias(label: &str, declaration: &str, rust_type: &str, should_compile: bool) {
	let out = Path::new(env!("OUT_DIR"));
	let commands: serde_json::Value =
		serde_json::from_slice(&fs::read(out.join("compile_commands.json")).expect("read compiler command"))
			.expect("parse compiler command");
	let arguments = commands[0]["arguments"].as_array().expect("compiler arguments");
	let compiler = arguments[0].as_str().expect("compiler path");
	// Reuse the native probe's flags and Windows SDK include paths, not its source file.
	let flags = arguments[1..arguments.len() - 2]
		.iter()
		.map(|argument| argument.as_str().expect("compiler argument"));
	let mut probe = generate::Probe::default();
	probe.add_source(&format!("pub type Word = {rust_type};"));
	let directory = out.join("generator-fixtures");
	fs::create_dir_all(&directory).expect("create fixture directory");
	let source = directory.join(format!("{label}.c"));
	fs::write(
		&source,
		format!("#include <stdint.h>\n{declaration}\n{}", probe.signatures),
	)
	.expect("write C fixture");
	let output = Command::new(compiler)
		.args(flags)
		.arg("-c")
		.arg(&source)
		.current_dir(&directory)
		.output()
		.expect("run native C compiler");
	assert_eq!(
		output.status.success(),
		should_compile,
		"{label}:\n{}\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn enum_aliases_match_the_native_compiler() {
	let (native, wrong_sign) = if cfg!(target_env = "msvc") {
		("i32", "u32")
	} else {
		("u32", "i32")
	};
	let positive = "typedef enum { POSITIVE = 1 } Word;";
	check_alias("positive_enum", positive, native, true);
	check_alias("negative_enum", "typedef enum { NEGATIVE = -1 } Word;", "i32", true);
	check_alias("enum_wrong_sign", positive, wrong_sign, false);
	check_alias("enum_wrong_width", positive, "i64", false);
}

#[test]
fn integer_promotion_does_not_hide_wrong_aliases() {
	check_alias("signed_byte", "typedef int8_t Word;", "i8", true);
	check_alias("unsigned_byte", "typedef uint8_t Word;", "u8", true);
	check_alias("signed_wide", "typedef int64_t Word;", "i64", true);
	check_alias("unsigned_wide", "typedef uint64_t Word;", "u64", true);
	check_alias("promoted_byte", "typedef uint8_t Word;", "i32", false);
	check_alias("wrong_sign", "typedef uint32_t Word;", "i32", false);
	check_alias("wrong_width", "typedef int64_t Word;", "i32", false);
	check_alias("floating_point", "typedef float Word;", "i32", false);
}
