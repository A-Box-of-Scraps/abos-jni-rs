//! Opt-in integration tests requiring an installed, unmodified Android API 35 SDK.
use abos_jbindgen::{AndroidSdk, Bindings, Builder};
use std::process::{Command, Output};

#[test]
#[ignore = "requires ANDROID_HOME or ANDROID_SDK_ROOT with platforms;android-35"]
fn generates_bundle_from_installed_api_35() -> Result<(), Box<dyn std::error::Error>> {
	let sdk: AndroidSdk = AndroidSdk::from_env(35)?;
	sdk.get_android_jar()?;
	sdk.get_stubs_src_jar()?;
	let bindings: Bindings = Builder::new()
		.input_android_sdk(35, vec!["android.os.Bundle".to_owned()])
		.generate()?;
	assert_eq!(bindings.len(), 1);
	let code: String = bindings.to_string();
	assert!(code.contains("pub Bundle => \"android.os.Bundle\""));
	assert!(code.contains("fn put_byte(key: JString, value: jbyte)"));
	assert!(code.contains("fn get_bundle(key: JString) -> crate::android::os::Bundle"));
	assert!(code.contains("#[deprecated]"));

	let output: Output = Command::new(env!("CARGO_BIN_EXE_abos-jbindgen"))
		.args(["android", "--api-level", "35", "--pattern", "android.os.Bundle"])
		.output()?;
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert_eq!(String::from_utf8(output.stdout)?.trim_end(), code.trim_end());
	Ok(())
}
