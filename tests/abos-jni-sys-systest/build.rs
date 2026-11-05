#[path = "support/generate.rs"]
mod generate;
#[path = "support/types.rs"]
mod types;

use std::{env, fs, path::PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=JAVA_HOME");
	assert_eq!(
		env::var("HOST"),
		env::var("TARGET"),
		"The JNI ABI probe requires a host JDK and native build"
	);
	let home = PathBuf::from(env::var_os("JAVA_HOME").expect("Set JAVA_HOME to a host JDK 24 or newer"));
	let platform = match env::var("CARGO_CFG_TARGET_OS").expect("target OS").as_str() {
		"linux" => "linux",
		"macos" => "darwin",
		"windows" => "win32",
		other => panic!("Unsupported JNI ABI probe host: {other}"),
	};
	let include = home.join("include");
	let platform_include = include.join(platform);
	for header in [include.join("jni.h"), platform_include.join("jni_md.h")] {
		assert!(header.is_file(), "Missing JDK header: {}", header.display());
		println!("cargo:rerun-if-changed={}", header.display());
	}
	let mut probe = generate::Probe::default();
	for source in [
		"../../crates/abos-jni-sys/src/api/types.rs",
		"../../crates/abos-jni-sys/src/api/constants.rs",
		"../../crates/abos-jni-sys/src/api/invocation.rs",
		"../../crates/abos-jni-sys/src/api/native.rs",
	] {
		println!("cargo:rerun-if-changed={source}");
		probe.add_source(&fs::read_to_string(source).expect("read JNI bindings"));
	}
	let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
	fs::write(out.join("signatures.h"), probe.signatures).expect("write C type checks");
	fs::write(out.join("measurements.inc"), probe.measurements).expect("write C measurements");
	let expected = probe.expected;
	fs::write(out.join("expected.rs"), quote::quote!(vec![#expected]).to_string()).expect("write Rust expectations");
	println!("cargo:rerun-if-changed=probe.c");
	cc::Build::new()
		.file("probe.c")
		.include(include)
		.include(platform_include)
		.include(out)
		.std("c11")
		.warnings_into_errors(true)
		.compile("abos_jni_probe");
}
