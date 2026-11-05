use crate::{Error, Result, internal::android::sdk};
use std::{env, path::PathBuf};

/// An installed Android SDK and selected API level.
#[derive(Debug, Clone)]
pub struct AndroidSdk {
	/// SDK installation root.
	pub sdk_path: PathBuf,
	/// Selected Android API level.
	pub api_level: u32,
}

impl AndroidSdk {
	/// Locate the SDK through `ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT`.
	///
	/// # Errors
	/// Returns an error if neither variable is set or the SDK directory is missing.
	pub fn from_env(api_level: u32) -> Result<Self> {
		let root: PathBuf = env::var_os("ANDROID_HOME")
			.or_else(|| env::var_os("ANDROID_SDK_ROOT"))
			.map(PathBuf::from)
			.ok_or_else(|| Error::AndroidSdk("set ANDROID_HOME or ANDROID_SDK_ROOT".to_owned()))?;
		Self::from_path(root, api_level)
	}

	/// Select an SDK root and API level without reading the environment.
	///
	/// # Errors
	/// Returns an error if the root is not a directory.
	pub fn from_path(sdk_path: PathBuf, api_level: u32) -> Result<Self> {
		if !sdk_path.is_dir() {
			return Err(Error::AndroidSdk(format!(
				"SDK directory not found: {}",
				sdk_path.display()
			)));
		}
		Ok(Self { sdk_path, api_level })
	}

	/// Locate the selected platform's compiled API JAR.
	///
	/// # Errors
	/// Returns an error if `android.jar` is missing.
	pub fn get_android_jar(&self) -> Result<PathBuf> {
		sdk::platform_file(self, "android.jar")
	}

	/// Locate the selected platform's Java source-stub JAR.
	///
	/// # Errors
	/// Returns an error if `android-stubs-src.jar` is missing.
	pub fn get_stubs_src_jar(&self) -> Result<PathBuf> {
		sdk::platform_file(self, "android-stubs-src.jar")
	}

	/// Return the platform JAR and locally available annotation dependencies.
	///
	/// # Errors
	/// Returns missing-platform or filesystem errors. Nothing is downloaded.
	pub fn get_classpath(&self) -> Result<Vec<PathBuf>> {
		sdk::classpath(self)
	}
}
