include!(concat!(env!("OUT_DIR"), "/net_uri_bindings.rs"));

use crate::test_error::{TestResult, require};
use android::net::AndroidUri;

pub fn test_net_uri<'local>(env: &mut jni::Env<'local>) -> TestResult {
	// Parse a URI using the generated bindings
	let uri_str = jni::objects::JString::from_str(env, "https://github.com/jni-rs/jni-rs")?;
	let uri = AndroidUri::parse(env, uri_str)?;

	// Get URI components
	let scheme = uri.get_scheme(env)?;
	let scheme_str = scheme.to_string();

	let host = uri.get_host(env)?;
	let host_str = host.to_string();
	let path = uri.get_path(env)?;
	let path_str = path.to_string();

	require(scheme_str == "https", format!("unexpected URI scheme: {scheme_str}"))?;
	require(host_str == "github.com", format!("unexpected URI host: {host_str}"))?;
	require(path_str == "/jni-rs/jni-rs", format!("unexpected URI path: {path_str}"))?;

	Ok(format!("Uri: {}://{}{}", scheme_str, host_str, path_str))
}
