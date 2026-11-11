//! Regression tests for Java compilation paths used by examples.

#![cfg(feature = "invocation")]

#[path = "../examples/utils/lib.rs"]
mod utils;

use std::{
	fs, io,
	path::{Path, PathBuf},
};
use tempfile::TempDir;

#[test]
fn restored_target_directory_does_not_need_a_cache_tag() -> io::Result<()> {
	let root: TempDir = tempfile::tempdir()?;
	let target: PathBuf = root.path().join("target");
	let output: PathBuf = target.join("debug/build/abos-jni-hash/out");
	fs::create_dir_all(&output)?;
	assert!(!target.join("CACHEDIR.TAG").exists());
	assert_eq!(utils::target_dir_from_output(&output), target);
	Ok(())
}

#[test]
fn unrecognized_output_layout_falls_back_to_target() {
	assert_eq!(
		utils::target_dir_from_output(Path::new("custom/build/hash/out")),
		PathBuf::from("target")
	);
}
