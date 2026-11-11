use crate::internal::{
	command::validate_classpath, discovery::parse_version, output::parse_classes, prepared::quote_source,
};
use std::path::{Path, PathBuf};

#[test]
fn parses_legacy_modern_and_early_access_versions() {
	for (text, expected) in [
		("javac 1.8.0_472", Some(8)),
		("javac 25.0.1", Some(25)),
		("javac 26-ea", Some(26)),
		("warning\njavac 21.0.2\n", Some(21)),
		("javac nonsense", None),
		("java 25", None),
		("", None),
	] {
		assert_eq!(parse_version(text), expected, "{text}");
	}
}

#[test]
fn parses_only_reported_classes_and_deduplicates() {
	let output = "[wrote /out/Foo.class]\n[wrote RegularFileObject[/out/Bar.class]]\n[wrote /out/Foo.class]\n[wrote /out/Foo.java]\n[checking Foo]\n[wrote broken.class\n";
	assert_eq!(
		parse_classes(output),
		vec![PathBuf::from("/out/Foo.class"), PathBuf::from("/out/Bar.class")]
	);
}

#[test]
fn quotes_argument_file_paths() -> crate::Result<()> {
	let quoted: String = quote_source(Path::new("source space/Example.java"))?;
	assert!(quoted.starts_with('"') && quoted.ends_with('"'));
	assert!(quoted.contains("source space"));
	Ok(())
}

#[cfg(unix)]
#[test]
fn escapes_special_characters_without_loss() -> crate::Result<()> {
	let quoted: String = quote_source(Path::new("a\\b\"c\nd\re\tf.java"))?;
	assert!(quoted.ends_with("a\\\\b\\\"c\\nd\\re\\tf.java\""));
	Ok(())
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_sources() {
	use std::{ffi::OsString, os::unix::ffi::OsStringExt};
	let path: PathBuf = PathBuf::from(OsString::from_vec(vec![0xff]));
	assert!(matches!(quote_source(&path), Err(crate::Error::InvalidSourcePath(_))));
}

#[test]
fn rejects_release_on_legacy_jdk_before_creating_output() -> crate::Result<()> {
	use crate::internal::{command::command, config::Config, prepared::Prepared};
	use std::sync::Arc;
	let root: tempfile::TempDir = tempfile::tempdir()?;
	let output: PathBuf = root.path().join("classes");
	let compiler = Prepared {
		config: Config {
			release: Some("8".to_owned()),
			..Config::default()
		},
		path: PathBuf::from("javac"),
		version: 8,
		sources: vec![PathBuf::from("Foo.java")],
		output_dir: output.clone(),
		output_src_dir: output.clone(),
		argument_file: Arc::new(tempfile::NamedTempFile::new()?),
	};
	assert!(matches!(command(&compiler), Err(crate::Error::Unsupported(_))));
	assert!(!output.exists());
	Ok(())
}

#[test]
fn rejects_java_classpath_separators_on_both_platforms() {
	for (separator, entry) in [(b';', "invalid;entry"), (b':', "invalid:entry")] {
		let paths: [PathBuf; 1] = [PathBuf::from(entry)];
		assert!(matches!(
			validate_classpath(&paths, separator),
			Err(crate::Error::InvalidClasspath(_))
		));
	}
}

#[test]
fn preserves_drive_letters_spaces_and_other_platforms_separators() -> crate::Result<()> {
	let windows: [PathBuf; 2] = [
		PathBuf::from(r"C:\dependencies with spaces"),
		PathBuf::from(r"D:\library.jar"),
	];
	validate_classpath(&windows, b';')?;
	let unix: [PathBuf; 2] = [
		PathBuf::from("/dependencies;with;semicolons"),
		PathBuf::from("/with spaces/library.jar"),
	];
	validate_classpath(&unix, b':')?;
	Ok(())
}
