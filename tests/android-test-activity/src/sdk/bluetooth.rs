include!(concat!(env!("OUT_DIR"), "/bluetooth_bindings.rs"));

use crate::test_error::TestResult;

pub fn test_bluetooth<'local>(env: &mut abos_jni::Env<'local>) -> TestResult {
	// Just verify that the JNI bindings are able to cache the class, methods, and fields..
	jni_init(env, &Default::default())?;

	Ok("Bluetooth class, method and field resolution passed".to_string())
}
