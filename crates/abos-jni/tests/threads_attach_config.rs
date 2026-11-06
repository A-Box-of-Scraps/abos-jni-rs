#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use std::thread::spawn;

use abos_jni::{AttachConfig, jni_sig, jni_str, objects::JString};

mod util;
use util::jvm;

// Tests that an `AttachConfig` can be used to configure the name of the JVM `Thread`
#[test]
fn attach_config() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = jvm();

	let thread: std::thread::JoinHandle<std::result::Result<(), abos_jni::errors::Error>> = spawn({
		move || {
			jvm.attach_current_thread_with_config(
				|| AttachConfig::new().thread_name(jni_str!("test-thread")),
				None,
				|env| -> abos_jni::errors::Result<_> {
					// Get the current Thread and query the name
					let thread: abos_jni::objects::JObject<'_> = env
						.call_static_method(
							jni_str!("java/lang/Thread"),
							jni_str!("currentThread"),
							jni_sig!("()Ljava/lang/Thread;"),
							&[],
						)
						.expect("static method call succeeds")
						.l()
						.expect("JNI result contains an object");
					let name: abos_jni::objects::JObject<'_> = env
						.call_method(thread, jni_str!("getName"), jni_sig!("()Ljava/lang/String;"), &[])?
						.l()?;
					let name: JString<'_> = env.cast_local::<JString>(name).expect("test operation succeeds");
					let name: abos_jni::strings::MUTF8Chars<'_, &JString<'_>> = name.mutf8_chars(env)?;
					assert_eq!(name.as_cstr(), c"test-thread");
					Ok(())
				},
			)
		}
	});

	thread
		.join()
		.expect("test worker completes without panicking")
		.expect("configured thread attaches and executes successfully");
}
