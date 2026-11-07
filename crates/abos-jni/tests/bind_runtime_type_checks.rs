#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
mod util;

use abos_jni_macros::bind_java_type;

bind_java_type! {
	rust_type = JDep,
	java_type = "java.util.ArrayList"
}

// The mapping deliberately disagrees with JDep's declared Java class.
bind_java_type! {
	rust_type = JTestType,
	java_type = "java.util.Vector",
	methods {
		fn test_method(dep: JDep) -> void,
	},
	type_map = {
		JDep => "java.util.HashMap", // This is WRONG! JDep is actually java.util.ArrayList
	}
}

#[test]
#[should_panic(expected = "Type mapping mismatch")]
fn test_incorrect_type_mapping_panics() {
	let result: std::result::Result<(), abos_jni::errors::Error> = util::attach_current_thread(|env| {
		// Type mapping checks run during API initialization.
		let loader: abos_jni::refs::LoaderContext<'_, '_> = abos_jni::refs::LoaderContext::None;
		JTestTypeAPI::get(env, &loader)?;

		Ok(())
	});

	result.expect("test operation succeeds");
}

bind_java_type! {
	rust_type = JDepCorrect,
	java_type = "java.util.LinkedList"
}

bind_java_type! {
	rust_type = JTestTypeCorrect,
	java_type = "java.util.Vector",
	type_map = {
		JDepCorrect => "java.util.LinkedList", // This is CORRECT!
	}
}

#[test]
fn test_correct_type_mapping_passes() {
	let result: std::result::Result<(), abos_jni::errors::Error> = util::attach_current_thread(|env| {
		let loader: abos_jni::refs::LoaderContext<'_, '_> = abos_jni::refs::LoaderContext::None;
		let _api: &JTestTypeCorrectAPI = JTestTypeCorrectAPI::get(env, &loader)?;

		Ok(())
	});

	result.expect("test operation succeeds");
}

#[repr(transparent)]
struct MyHandle(*mut std::os::raw::c_void);

// It's enough to check that this compiles
bind_java_type! {
	rust_type = JHandleType,
	java_type = "com.example.HandleType",
	type_map = {
		unsafe MyHandle => long,
	}
}
