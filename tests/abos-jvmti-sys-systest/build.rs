#[path = "../abos-jni-sys-systest/support/generate.rs"]
mod generate;
#[path = "../abos-jni-sys-systest/support/types.rs"]
mod types;

mod capabilities;

use std::{env, fs, path::PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=JAVA_HOME");
	assert_eq!(
		env::var("HOST"),
		env::var("TARGET"),
		"The ABI probe requires a native host JDK"
	);
	let home = PathBuf::from(env::var_os("JAVA_HOME").expect("Set JAVA_HOME to JDK 25"));
	let platform = match env::var("CARGO_CFG_TARGET_OS").expect("target OS").as_str() {
		"linux" => "linux",
		"macos" => "darwin",
		"windows" => "win32",
		other => panic!("Unsupported JVMTI ABI probe host: {other}"),
	};
	let include = home.join("include");
	for header in [
		include.join("jvmti.h"),
		include.join("jni.h"),
		include.join(platform).join("jni_md.h"),
	] {
		assert!(header.is_file(), "Missing JDK header: {}", header.display());
		println!("cargo:rerun-if-changed={}", header.display());
	}
	let mut probe = generate::Probe::default();
	for name in ["base", "constants", "types", "callbacks", "interface", "capabilities"] {
		let source = format!("../../crates/abos-jvmti-sys/src/api/{name}.rs");
		println!("cargo:rerun-if-changed={source}");
		probe.add_source(&fs::read_to_string(source).expect("read bindings"));
	}
	let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
	fs::write(out.join("signatures.h"), probe.signatures).expect("write C type checks");
	fs::write(out.join("measurements.inc"), probe.measurements).expect("write C measurements");
	let expected = probe.expected;
	fs::write(out.join("expected.rs"), quote::quote!(vec![#expected]).to_string()).expect("write Rust expectations");
	capabilities::generate(&out);
	println!("cargo:rerun-if-changed=probe.c");
	cc::Build::new()
		.file("probe.c")
		.include(&include)
		.include(include.join(platform))
		.include(out)
		.std("c11")
		.warnings_into_errors(true)
		.compile("abos_jvmti_probe");
}
