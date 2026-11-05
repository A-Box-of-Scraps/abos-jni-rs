include!(concat!(env!("OUT_DIR"), "/os_binder_bindings.rs"));

use crate::test_error::TestResult;

pub fn test_os_binder<'local>(env: &mut jni::Env<'local>) -> TestResult {
	// Just verify that the JNI bindings are able to cache the class, methods, and fields..
	jni_init(env, &Default::default())?;

	Ok("Binder class, method and field resolution passed".to_string())
}
