use super::{JVM_LIBRARY, find_library, home_from_output, locate_with};
use std::{
	ffi::OsString,
	fs, io,
	path::{Path, PathBuf},
};
use tempfile::TempDir;

fn create_library(home: &Path, layout: &str) -> io::Result<PathBuf> {
	let directory: PathBuf = home.join(layout);
	fs::create_dir_all(&directory)?;
	let library: PathBuf = directory.join(JVM_LIBRARY);
	fs::write(&library, [])?;
	Ok(library)
}

#[test]
fn finds_modern_legacy_and_custom_layouts() -> io::Result<()> {
	for layout in [
		"lib/server",
		"jre/lib/amd64/server",
		"jre/lib/i386/client",
		"bin/server",
		"jre/bin/client",
		"custom/runtime",
		"",
	] {
		let home: TempDir = tempfile::tempdir()?;
		let expected: PathBuf = create_library(home.path(), layout)?;
		assert_eq!(find_library(home.path())?, expected);
	}
	Ok(())
}

#[test]
fn explicit_home_does_not_use_fallback() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	let expected: PathBuf = create_library(home.path(), "lib/server")?;
	let actual: PathBuf = locate_with(Some(home.path().as_os_str().to_owned()), || {
		panic!("unexpected fallback")
	})?;
	assert_eq!(actual, expected);
	Ok(())
}

#[test]
fn empty_and_missing_home_use_fallback() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	let expected: PathBuf = create_library(home.path(), "lib/server")?;
	for value in [None, Some(OsString::new())] {
		assert_eq!(locate_with(value, || Ok(home.path().to_path_buf()))?, expected);
	}
	Ok(())
}

#[test]
fn invalid_explicit_home_does_not_use_fallback() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	for path in [home.path().to_path_buf(), home.path().join("missing")] {
		let error: io::Error = locate_with(Some(path.clone().into_os_string()), || panic!("unexpected fallback"))
			.expect_err("a home without a JVM must fail");
		assert_eq!(error.kind(), io::ErrorKind::NotFound);
		assert!(error.to_string().contains(JVM_LIBRARY));
		assert!(error.to_string().contains(&path.display().to_string()));
	}
	Ok(())
}

#[test]
fn propagates_discovery_errors() {
	let error: io::Error = locate_with(None, || {
		Err(io::Error::new(io::ErrorKind::PermissionDenied, "cannot run discovery"))
	})
	.expect_err("discovery must fail");
	assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn ignores_directories_and_wrong_library_names() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	fs::create_dir(home.path().join(JVM_LIBRARY))?;
	fs::write(home.path().join(format!("{JVM_LIBRARY}.backup")), [])?;
	assert_eq!(
		find_library(home.path()).expect_err("not a JVM library").kind(),
		io::ErrorKind::NotFound
	);
	Ok(())
}

#[test]
fn selects_first_layout_deterministically() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	let _ = create_library(home.path(), "z/server")?;
	let expected: PathBuf = create_library(home.path(), "a/server")?;
	assert_eq!(find_library(home.path())?, expected);
	Ok(())
}

#[test]
fn command_output_uses_first_java_path() -> io::Result<()> {
	let home: TempDir = tempfile::tempdir()?;
	let bin: PathBuf = home.path().join("bin");
	fs::create_dir(&bin)?;
	let java: PathBuf = bin.join("java");
	fs::write(&java, [])?;
	let output: String = format!("{}\r\n/nonexistent/other/java\r\n", java.display());
	assert_eq!(
		home_from_output(output.as_bytes(), false)?,
		fs::canonicalize(home.path())?
	);
	Ok(())
}

#[test]
fn macos_output_is_already_a_home() -> io::Result<()> {
	let root: TempDir = tempfile::tempdir()?;
	let home: PathBuf = root.path().join("JDK with spaces/Contents/Home");
	fs::create_dir_all(&home)?;
	let output: String = format!("{}\n", home.display());
	assert_eq!(home_from_output(output.as_bytes(), true)?, fs::canonicalize(home)?);
	Ok(())
}

#[test]
fn rejects_empty_and_invalid_command_output() {
	for output in [b"".as_slice(), b"\n", b"  \r\n"] {
		assert_eq!(
			home_from_output(output, false).expect_err("empty output").kind(),
			io::ErrorKind::NotFound
		);
	}
	assert_eq!(
		home_from_output(&[0xff], false).expect_err("invalid UTF-8").kind(),
		io::ErrorKind::InvalidData
	);
}

#[cfg(unix)]
#[test]
fn resolves_relative_java_symlinks() -> io::Result<()> {
	let root: TempDir = tempfile::tempdir()?;
	let home: PathBuf = root.path().join("jdk");
	fs::create_dir_all(home.join("bin"))?;
	fs::write(home.join("bin/java"), [])?;
	let link: PathBuf = root.path().join("java");
	std::os::unix::fs::symlink("jdk/bin/java", &link)?;
	assert_eq!(
		home_from_output(link.to_string_lossy().as_bytes(), false)?,
		fs::canonicalize(home)?
	);
	Ok(())
}

#[cfg(unix)]
#[test]
fn follows_linked_libraries_without_looping() -> io::Result<()> {
	let root: TempDir = tempfile::tempdir()?;
	let home: PathBuf = root.path().join("jdk");
	fs::create_dir(&home)?;
	std::os::unix::fs::symlink(&home, home.join("a-cycle"))?;
	std::os::unix::fs::symlink("missing", home.join("broken"))?;
	assert!(find_library(&home).is_err());
	let runtime: PathBuf = root.path().join("runtime");
	let _ = create_library(&runtime, "server")?;
	std::os::unix::fs::symlink(&runtime, home.join("lib"))?;
	assert_eq!(find_library(&home)?, home.join("lib/server").join(JVM_LIBRARY));
	Ok(())
}

#[test]
fn supports_unicode_java_home() -> io::Result<()> {
	let root: TempDir = tempfile::tempdir()?;
	let home: PathBuf = root.path().join("JDK 日本語");
	let expected: PathBuf = create_library(&home, "lib/server")?;
	assert_eq!(
		locate_with(Some(home.into_os_string()), || panic!("unexpected fallback"))?,
		expected
	);
	Ok(())
}

// macOS filesystems reject invalid UTF-8 filenames before discovery can run.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn supports_non_utf8_java_home() -> io::Result<()> {
	use std::os::unix::ffi::OsStringExt;
	let root: TempDir = tempfile::tempdir()?;
	let home: PathBuf = root.path().join(OsString::from_vec(vec![b'j', 0xff]));
	let expected: PathBuf = create_library(&home, "lib/server")?;
	assert_eq!(
		locate_with(Some(home.into_os_string()), || panic!("unexpected fallback"))?,
		expected
	);
	Ok(())
}
