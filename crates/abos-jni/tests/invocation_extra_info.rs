#![allow(unsafe_code, reason = "Exercise explicitly unsafe JVM callback registration.")]
#![allow(missing_docs, reason = "Integration test, not library API.")]
#![cfg(feature = "invocation")]

use std::{ffi::c_void, process::Command};

use abos_jni::{InitArgsBuilder, JavaVM, jni_sig, jni_str, sys::jint};

const JAVA_EXIT_STATUS: jint = 37;
const HOOK_EXIT_STATUS: i32 = 73;
const CHILD_ENV: &str = "ABOS_JNI_EXTRA_INFO_EXIT_HOOK_CHILD";

#[allow(
	clippy::exit,
	reason = "Terminate only the child process to report that the JVM invoked the hook."
)]
extern "system" fn exit_hook(status: jint) {
	// A distinct status proves that the JVM invoked this hook with the correct argument.
	std::process::exit(if status == JAVA_EXIT_STATUS {
		HOOK_EXIT_STATUS
	} else {
		74
	});
}

#[test]
fn exit_hook_is_invoked_by_jvm() {
	if std::env::var_os(CHILD_ENV).is_none() {
		// System.exit terminates the process, so the JVM must run in a separate test process.
		let output: std::process::Output = Command::new(std::env::current_exe().expect("test executable"))
			.args(["--exact", "exit_hook_is_invoked_by_jvm", "--nocapture"])
			.env(CHILD_ENV, "1")
			.output()
			.expect("spawn callback test process");
		assert_eq!(
			output.status.code(),
			Some(HOOK_EXIT_STATUS),
			"JVM did not call exit hook: stdout={} stderr={}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
		return;
	}

	// Safety: The hook has the JVM's exit ABI, static code lifetime, no shared state, and
	// exits without unwinding. It remains valid after JavaVM::new consumes the arguments.
	let args: abos_jni::InitArgs<'_> =
		unsafe { InitArgsBuilder::new().option_with_extra_info("exit", exit_hook as *mut c_void) }
			.build()
			.expect("valid JVM arguments");
	let jvm: JavaVM = JavaVM::new(args).expect("start JVM with exit hook");
	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		env.call_static_method(
			jni_str!("java/lang/System"),
			jni_str!("exit"),
			jni_sig!("(I)V"),
			&[JAVA_EXIT_STATUS.into()],
		)?;
		Ok(())
	})
	.expect("System.exit succeeds");
	panic!("System.exit unexpectedly returned");
}
