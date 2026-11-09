//! Regenerate the Windows API subset used by abos-jni.

use std::path::Path;

fn main() {
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let output = manifest_dir.join("../../crates/abos-jni/src/internal/windows_sys.rs");
	let config = manifest_dir.join("bindings.config");
	windows_bindgen::bindgen([
		"--out",
		output.to_str().expect("UTF-8 output path"),
		"--etc",
		config.to_str().expect("UTF-8 config path"),
	]);
}
