#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

#[test]
fn test_builtin_bindings() {
	let jvm = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		abos_jni::__test_bindings_init(env, &Default::default());
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
