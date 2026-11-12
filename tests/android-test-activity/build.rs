//! Generate Android application bindings using an explicitly installed SDK.
use abos_jbindgen::{AndroidSdk, Builder};
use std::{env, path::PathBuf};

type BuildResult = Result<(), Box<dyn std::error::Error>>;

fn main() -> BuildResult {
	for name in ["ANDROID_HOME", "ANDROID_SDK_ROOT", "JAVA_HOME"] {
		println!("cargo:rerun-if-env-changed={name}");
	}
	let sdk = AndroidSdk::from_env(35)?;
	let android_jar = sdk.get_android_jar()?;
	println!("cargo:rerun-if-changed={}", android_jar.display());
	let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
	let sources = PathBuf::from("app/src/main/java");
	println!("cargo:rerun-if-changed={}", sources.display());
	Builder::new()
		.input_sources(
			vec![sources],
			vec![android_jar.clone()],
			vec!["io.github.jni_rs.jbindgen.testactivity.TestActivity".to_owned()],
		)
		.generate()?
		.write_to_file(output.join("test_activity_bindings.rs"))?;
	for (feature, module, patterns, prefix, skip) in [
		(
			"SDK_UTIL_TIME_UTILS",
			"util_time_utils",
			&["android.util.TimeUtils", "android.icu.util.TimeZone"][..],
			None,
			// The SDK stub includes this package-private constructor, but ART blocks JNI access.
			Some("Landroid/util/TimeUtils;-><init>()V"),
		),
		(
			"SDK_OS_BUILD",
			"os_build",
			&["android.os.Build"][..],
			Some("Android"),
			None,
		),
		(
			"SDK_OS_BINDER",
			"os_binder",
			&["android.os.Binder"][..],
			Some("Android"),
			None,
		),
		(
			"SDK_BLUETOOTH",
			"bluetooth",
			&["android.bluetooth.le.*"][..],
			Some("A"),
			None,
		),
		(
			"SDK_CONTENT_INTENT",
			"content_intent",
			&["android.content.Intent"][..],
			Some("Android"),
			Some("Landroid/content/Intent;->toURI()Ljava/lang/String;"),
		),
		(
			"SDK_NET_URI",
			"net_uri",
			&["android.net.Uri"][..],
			Some("Android"),
			None,
		),
	] {
		if env::var_os(format!("CARGO_FEATURE_{feature}")).is_none() {
			continue;
		}
		let mut builder = Builder::new()
			.root_path(format!("crate::sdk::{module}"))
			// These tests use the platform JAR without source documentation. Explicit
			// skips exclude stub members that Android blocks at runtime.
			.input_jar(&android_jar)
			.patterns(patterns.iter().copied())
			.generate_native_interfaces(false);
		if let Some(prefix) = prefix {
			builder = builder.name_prefix(prefix);
		}
		if let Some(signature) = skip {
			builder = builder.skip_signature(signature);
		}
		builder
			.generate()?
			.write_to_file(output.join(format!("{module}_bindings.rs")))?;
	}
	Ok(())
}
