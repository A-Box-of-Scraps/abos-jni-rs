#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

#[path = "util/fixtures.rs"]
mod fixtures;

use std::{
	fs::{self, File, FileTimes},
	path::{Path, PathBuf},
	process::{Command, Stdio},
	time::{Duration, SystemTime},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source_root(root: &Path) -> std::io::Result<PathBuf> {
	let sources: PathBuf = root.join("sources");
	fs::create_dir(&sources)?;
	fs::write(
		sources.join("Example.java"),
		"public class Example { static class Inner {} }",
	)?;
	Ok(sources)
}

#[test]
fn reuses_classes_and_invalidates_changed_added_and_removed_sources() -> TestResult {
	let root: tempfile::TempDir = tempfile::tempdir()?;
	let sources: PathBuf = source_root(root.path())?;
	let cache: PathBuf = root.path().join("cache");
	let first: PathBuf = fixtures::compile_fixtures(&sources, &cache)?;
	assert!(first.join("Example$Inner.class").is_file());
	let class: PathBuf = first.join("Example.class");
	let bytes: Vec<u8> = fs::read(&class)?;
	let old: SystemTime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
	File::options()
		.write(true)
		.open(&class)?
		.set_times(FileTimes::new().set_modified(old))?;
	let modified: SystemTime = fs::metadata(&class)?.modified()?;
	assert_eq!(fixtures::compile_fixtures(&sources, &cache)?, first);
	assert_eq!(fs::metadata(&class)?.modified()?, modified);

	// Equal-length edits must invalidate by content, not size or timestamp granularity.
	fs::write(
		sources.join("Example.java"),
		"public class Example { static class Other {} }",
	)?;
	let changed: PathBuf = fixtures::compile_fixtures(&sources, &cache)?;
	assert_ne!(changed, first);
	assert!(changed.join("Example$Other.class").is_file());
	assert!(!changed.join("Example$Inner.class").exists());
	assert_eq!(fs::read(&class)?, bytes);

	fs::create_dir(sources.join("nested"))?;
	fs::write(
		sources.join("nested/Added.java"),
		"package nested; public class Added {}",
	)?;
	let added: PathBuf = fixtures::compile_fixtures(&sources, &cache)?;
	assert_ne!(added, changed);
	assert!(added.join("nested/Added.class").is_file());
	fs::remove_file(sources.join("nested/Added.java"))?;
	assert_eq!(fixtures::compile_fixtures(&sources, &cache)?, changed);
	Ok(())
}

#[test]
fn failed_compilation_does_not_publish_classes() -> TestResult {
	let root: tempfile::TempDir = tempfile::tempdir()?;
	let sources: PathBuf = source_root(root.path())?;
	let cache: PathBuf = root.path().join("cache");
	fs::write(sources.join("Broken.java"), "invalid Java")?;
	assert!(fixtures::compile_fixtures(&sources, &cache).is_err());
	for entry in fs::read_dir(&cache)? {
		assert!(!entry?.file_type()?.is_dir());
	}
	fs::remove_file(sources.join("Broken.java"))?;
	let output: PathBuf = fixtures::compile_fixtures(&sources, &cache)?;
	assert!(output.join("Example.class").is_file());
	Ok(())
}

#[test]
fn concurrent_processes_share_complete_classes() -> TestResult {
	let root: tempfile::TempDir = tempfile::tempdir()?;
	source_root(root.path())?;
	let mut children: Vec<std::process::Child> = Vec::new();
	for _ in 0..4 {
		children.push(
			Command::new(std::env::current_exe()?)
				.args(["--ignored", "--exact", "fixture_cache_child"])
				.env("ABOS_JNI_FIXTURE_CACHE_TEST", root.path())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.spawn()?,
		);
	}
	for child in children {
		let output: std::process::Output = child.wait_with_output()?;
		assert!(
			output.status.success(),
			"{}{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}
	let mut directories: usize = 0;
	for entry in fs::read_dir(root.path().join("cache"))? {
		if entry?.file_type()?.is_dir() {
			directories += 1;
		}
	}
	assert_eq!(directories, 1, "only one complete cache entry should remain");
	Ok(())
}

#[test]
#[ignore = "Run by concurrent_processes_share_complete_classes in isolated processes"]
fn fixture_cache_child() -> TestResult {
	let root: PathBuf = std::env::var_os("ABOS_JNI_FIXTURE_CACHE_TEST")
		.expect("parent sets fixture directory")
		.into();
	let output: PathBuf = fixtures::compile_fixtures(&root.join("sources"), &root.join("cache"))?;
	for name in ["Example", "Example$Inner"] {
		assert!(fs::read(output.join(format!("{name}.class")))?.starts_with(&[0xca, 0xfe, 0xba, 0xbe]));
	}
	Ok(())
}
