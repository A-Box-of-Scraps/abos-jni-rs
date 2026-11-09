#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
//! Tests for `catch_unwind = false` behavior.
//!
//! This test file verifies the behavior difference between `catch_unwind =
//! false` and `catch_unwind = true` (the default).
//!
//! When `catch_unwind = false`:
//! - Native method implementations use `EnvUnowned::with_env_no_catch`
//! - Panics are NOT caught and converted to Java exceptions
//!
//! When `catch_unwind = true` (default):
//! - Native method wrappers use `EnvUnowned::with_env` (including `catch_unwind`)
//! - Panics are converted to Java exceptions via the error policy
//!
//! Testing `catch_unwind = false` is tricky because the native method
//! represents an extern "system" FFI boundary which panics cannot cross without
//! aborting the process. To work around this, we spawn a separate test-executable
//! process to run the code that can abort so we can check the exit status.

mod native_methods_utils;
mod util;

use abos_jni::errors::Error;
use abos_jni::sys::jint;
use abos_jni::{Env, native_method};
use rusty_fork::rusty_fork_test;
use std::panic;

// Test: catch_unwind = false (panics are NOT caught by error policy)

// This test is designed to abort when run
native_method_test! {
	#[ignore = "This is run in a separate test process by test_catch_unwind_false_causes_abort"]
	test_name: test_catch_unwind_false_causes_abort_impl,
	java_class: "com/example/TestNativeCatchUnwind.java",
	methods: |class| &[
		native_method! {
			extern fn method_that_panics0() -> jint,
			fn = method_that_panics_no_catch,
			java_type = "com.example.TestNativeCatchUnwind",
			catch_unwind = false,  // This will cause abort when panic tries to escape
		},
	],
	test_body: |env, class| {
		let obj = new_object!(env, class)?;

		// Call the method - this will panic and abort because catch_unwind = false
		// SAFETY: This thread owns the Env, and the live arguments match the fixture's exported JNI signature.
		unsafe {
			let unowned = abos_jni::EnvUnowned::from_raw(env.get_raw());
			let _ret = Java_com_example_TestNativeCatchUnwind_methodThatPanics0__(unowned, obj.as_raw());
		}

		eprintln!("ERROR: Should have aborted!");
		#[expect(clippy::exit, reason = "Terminate the subprocess fixture without introducing a second panic.")]
		std::process::exit(1);
		#[allow(unreachable_code, reason = "The test closure must type-check as returning Result even though this fixture exits.")]
		Ok(())
	}
}

fn method_that_panics_no_catch<'local>(
	_env: &mut Env<'local>,
	_this: abos_jni::objects::JObject<'local>,
) -> Result<jint, Error> {
	panic!("This panic should cause abort because catch_unwind = false");
}

unsafe extern "system" {
	fn Java_com_example_TestNativeCatchUnwind_methodThatPanics0__(
		env: abos_jni::EnvUnowned,
		this: abos_jni::sys::jobject,
	) -> jint;
}

#[test]
fn test_catch_unwind_false_causes_abort() {
	// Run the fixture test that has catch_unwind = false and panics.
	// The panic should cause the process to abort because it tries to
	// escape through an extern "system" FFI boundary that cannot unwind.

	let executable: std::path::PathBuf = std::env::current_exe().expect("locate test executable");
	let mut cmd = std::process::Command::new(executable);
	cmd.arg("--include-ignored")
		.arg("--exact")
		.arg("test_catch_unwind_false_causes_abort_impl")
		.arg("--nocapture");

	let output: std::process::Output = cmd.output().expect("Failed to execute fixture test");

	assert!(!output.status.success(), "Expected process to abort/fail");

	let stderr: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&output.stderr);
	let stdout: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&output.stdout);
	let combined: String = format!("{stdout}{stderr}");

	assert!(
		combined.contains("This panic should cause abort because catch_unwind = false"),
		"Expected panic message in output, got stdout: {stdout}\nstderr: {stderr}"
	);

	assert!(
		combined.contains("test test_catch_unwind_false_causes_abort_impl ... FAILED"),
		"Expected test failure in output, got: {combined}"
	);
}

// Test: catch_unwind = true (default - panics ARE caught by error policy)

native_method_test! {
	test_name: test_catch_unwind_true_catches_panic,
	java_class: "com/example/TestNativeCatchUnwind.java",
	methods: |class| &[
		native_method! {
			extern fn method_that_panics1() -> jint,
			fn = method_that_panics_with_catch,
			java_type = "com.example.TestNativeCatchUnwind",
			catch_unwind = true,  // Explicit, but this is the default
		},
	],
	expect_exception: "Rust panic: This panic SHOULD be caught by error policy",
	test_body: |env, class| {
		let obj = new_object!(env, class)?;

		// The wrapper must translate the panic to a Java exception before it reaches Rust callers.
		// SAFETY: This thread owns the Env, and the live arguments match the fixture's exported JNI signature.
		let result = panic::catch_unwind(panic::AssertUnwindSafe(|| unsafe {
			let unowned = abos_jni::EnvUnowned::from_raw(env.get_raw());
			Java_com_example_TestNativeCatchUnwind_methodThatPanics1__(unowned, obj.as_raw())
		}));

		assert!(result.is_ok(), "Expected panic to be caught by error policy when catch_unwind = true");

		Ok(())
	}
}

fn method_that_panics_with_catch<'local>(
	_env: &mut Env<'local>,
	_this: abos_jni::objects::JObject<'local>,
) -> Result<jint, Error> {
	panic!("This panic SHOULD be caught by error policy");
}

unsafe extern "system" {
	fn Java_com_example_TestNativeCatchUnwind_methodThatPanics1__(
		env: abos_jni::EnvUnowned,
		this: abos_jni::sys::jobject,
	) -> jint;
}
