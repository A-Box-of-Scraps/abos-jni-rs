include!(concat!(env!("OUT_DIR"), "/os_build_bindings.rs"));

use crate::test_error::{TestResult, require};
use android::os::AndroidBuild;

pub fn test_os_build<'local>(env: &mut abos_jni::Env<'local>) -> TestResult {
	// Use the generated AndroidBuild bindings to access static fields
	let brand = AndroidBuild::BRAND(env)?;
	let brand_str = brand.to_string();

	let model = AndroidBuild::MODEL(env)?;
	let model_str = model.to_string();

	let device = AndroidBuild::DEVICE(env)?;
	let device_str = device.to_string();

	let manufacturer = AndroidBuild::MANUFACTURER(env)?;
	let manufacturer_str = manufacturer.to_string();

	let version_release = android::os::AndroidBuildVERSION::RELEASE(env)?;
	let version_str = version_release.to_string();

	let sdk_int = android::os::AndroidBuildVERSION::SDK_INT(env)?;

	require(sdk_int >= 31, format!("device API {sdk_int} is below minSdk 31"))?;
	for (name, value) in [
		("brand", &brand_str),
		("model", &model_str),
		("device", &device_str),
		("manufacturer", &manufacturer_str),
		("release", &version_str),
	] {
		require(!value.trim().is_empty(), format!("Build {name} is empty"))?;
	}

	let result = format!("Brand: {}, Model: {}, Device: {}", brand_str, model_str, device_str);

	Ok(result)
}
