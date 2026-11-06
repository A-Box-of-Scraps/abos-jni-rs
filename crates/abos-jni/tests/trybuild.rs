#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

#[test]
fn ui() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/fail/*.rs");

	t.compile_fail("tests/ui/bind_java_type/fail/*.rs");
	t.pass("tests/ui/bind_java_type/pass/*.rs");
	t.compile_fail("tests/ui/native_method/fail/*.rs");
	t.pass("tests/ui/native_method/pass/*.rs");
}
