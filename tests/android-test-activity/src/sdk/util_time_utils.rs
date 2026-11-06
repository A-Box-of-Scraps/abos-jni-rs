include!(concat!(env!("OUT_DIR"), "/util_time_utils_bindings.rs"));

use crate::test_error::{TestResult, require};

pub fn test_time_utils<'local>(env: &mut abos_jni::Env<'local>) -> TestResult {
	// Just verify that the JNI bindings are able to cache the class, methods, and fields..
	jni_init(env, &Default::default())?;

	// Get timezone database version using the generated bindings
	let tz_version = android::util::TimeUtils::get_time_zone_database_version(env)?;
	let tz_version_str = tz_version.to_string();

	// Get default timezone using android.icu.util.TimeZone bindings
	let default_tz = android::icu::util::TimeZone::get_default(env)?;

	// Get timezone ID
	let tz_id = default_tz.get_id(env)?;
	let tz_id_str = tz_id.to_string();

	// Get timezone display name
	let tz_display_name = default_tz.get_display_name(env)?;
	let tz_display_name_str = tz_display_name.to_string();

	for (name, value) in [
		("database version", &tz_version_str),
		("ID", &tz_id_str),
		("display name", &tz_display_name_str),
	] {
		require(!value.trim().is_empty(), format!("timezone {name} is empty"))?;
	}

	Ok(format!("TimeZone: {} ({})", tz_id_str, tz_version_str))
}
