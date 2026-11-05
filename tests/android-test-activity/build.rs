//! Generate Android application bindings using an explicitly installed SDK.
use abos_jbindgen::{AndroidSdk, Builder};
use std::{env, path::PathBuf};

type BuildResult = Result<(), Box<dyn std::error::Error>>;

fn main() -> BuildResult {
	for name in ["ANDROID_HOME", "ANDROID_SDK_ROOT", "ABOS_HIDDENAPI_FLAGS"] {
		println!("cargo:rerun-if-env-changed={name}");
	}
	let sdk = AndroidSdk::from_env(35)?;
	let android_jar = sdk.get_android_jar()?;
	let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
	let source = PathBuf::from("app/src/main/java/io/github/jni_rs/jbindgen/testactivity/TestActivity.java");
	abos_javac::Build::new()
		.file(&source)
		.classpath(&android_jar)
		.output_dir(output.join("classes"))
		.encoding("UTF-8")
		.emit_rerun_if_changed(true)
		.try_compile()?;
	Builder::new()
		.input_sources(
			vec![source],
			vec![android_jar],
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
			None,
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
			.input_android_sdk(35, patterns.iter().map(|pattern| (*pattern).to_owned()).collect());
		if let Some(prefix) = prefix {
			builder = builder.name_prefix(prefix);
		}
		if let Some(signature) = skip {
			builder = builder.skip_signature(signature);
		}
		if let Some(flags) = env::var_os("ABOS_HIDDENAPI_FLAGS") {
			let path = PathBuf::from(flags);
			if !path.is_file() {
				return Err("ABOS_HIDDENAPI_FLAGS must name an existing CSV file".into());
			}
			println!("cargo:rerun-if-changed={}", path.display());
			builder = builder.hiddenapi_flags(path);
		}
		builder
			.generate()?
			.write_to_file(output.join(format!("{module}_bindings.rs")))?;
	}
	Ok(())
}
