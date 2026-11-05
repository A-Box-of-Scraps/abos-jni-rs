use crate::{AndroidSdk, Error, Result};
use std::{
	fs,
	path::{Path, PathBuf},
};

pub fn platform_file(sdk: &AndroidSdk, name: &str) -> Result<PathBuf> {
	let path: PathBuf = sdk
		.sdk_path
		.join("platforms")
		.join(format!("android-{}", sdk.api_level))
		.join(name);
	if !path.is_file() {
		return Err(Error::AndroidSdk(format!(
			"platform file not found: {}",
			path.display()
		)));
	}
	Ok(path)
}

pub fn classpath(sdk: &AndroidSdk) -> Result<Vec<PathBuf>> {
	let mut entries: Vec<PathBuf> = vec![sdk.get_android_jar()?];
	let support: PathBuf = sdk.sdk_path.join("tools/support/annotations.jar");
	if support.is_file() {
		entries.push(support);
	}
	for suffix in [
		"extras/m2repository/androidx/annotation/annotation",
		"extras/android/m2repository/com/android/support/support-annotations",
	] {
		if let Some(jar) = annotation_jar(&sdk.sdk_path.join(suffix))? {
			entries.push(jar);
		}
	}
	Ok(entries)
}

fn annotation_jar(root: &Path) -> Result<Option<PathBuf>> {
	if !root.is_dir() {
		return Ok(None);
	}
	let mut versions: Vec<PathBuf> = Vec::new();
	for entry in fs::read_dir(root)? {
		let entry: fs::DirEntry = entry?;
		if entry.file_type()?.is_dir() {
			versions.push(entry.path());
		}
	}
	versions.sort_by_cached_key(|path| {
		path.file_name()
			.unwrap_or_default()
			.to_string_lossy()
			.split('.')
			.map(|part| part.parse::<u64>().unwrap_or(0))
			.collect::<Vec<u64>>()
	});
	let Some(latest) = versions.last() else {
		return Ok(None);
	};
	let mut jars: Vec<PathBuf> = Vec::new();
	for entry in fs::read_dir(latest)? {
		let path: PathBuf = entry?.path();
		if path.is_file()
			&& path.extension().is_some_and(|ext| ext == "jar")
			&& !path.to_string_lossy().contains("-sources")
		{
			jars.push(path);
		}
	}
	jars.sort();
	Ok(jars.into_iter().next())
}
