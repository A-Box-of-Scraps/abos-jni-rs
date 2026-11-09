//! Embedded Java annotations for customizing generated Rust bindings.

#[doc = include_str!("../CHANGELOG.md")]
#[doc = "\n\n[Unreleased]: #unreleased\n"]
pub mod changelog {}

use std::{
	fs, io,
	path::{Path, PathBuf},
};

/// Java package containing the annotations, retained for source compatibility.
pub const PACKAGE: &str = "io.github.jni_rs.jbindgen";

/// Annotation source names and UTF-8 contents.
pub const ANNOTATION_FILES: &[(&str, &str)] = &[
	(
		"RustName.java",
		include_str!("../java/io/github/jni_rs/jbindgen/RustName.java"),
	),
	(
		"RustPrimitive.java",
		include_str!("../java/io/github/jni_rs/jbindgen/RustPrimitive.java"),
	),
	(
		"RustSkip.java",
		include_str!("../java/io/github/jni_rs/jbindgen/RustSkip.java"),
	),
	(
		"package-info.java",
		include_str!("../java/io/github/jni_rs/jbindgen/package-info.java"),
	),
];

/// Write annotation sources under their package directory and return their paths.
/// Existing files with these names are replaced.
///
/// # Errors
/// Returns filesystem errors when creating directories or writing sources.
pub fn write_sources(root: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
	let directory: PathBuf = root.as_ref().join("io/github/jni_rs/jbindgen");
	fs::create_dir_all(&directory)?;
	ANNOTATION_FILES
		.iter()
		.map(|(name, source)| {
			let path: PathBuf = directory.join(name);
			fs::write(&path, source)?;
			Ok(path)
		})
		.collect()
}
