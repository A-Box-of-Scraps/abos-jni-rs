//! Configure platform and compiler support.
use rustc_version::{Version, version};

// Gate unsafe-attribute syntax for consumers on Rust versions older than 1.82.
fn main() {
	println!("cargo::rustc-check-cfg=cfg(has_unsafe_attr)");

	let v: Version = version().expect("query rustc version");
	if v >= Version::new(1, 82, 0) {
		println!("cargo:rustc-cfg=has_unsafe_attr");
	}
}
