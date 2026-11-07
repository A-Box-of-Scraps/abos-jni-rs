#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni_macros::bind_java_type;

bind_java_type! { CustomType1 => com.example.CustomType1 }
bind_java_type! { CustomType2 => com.example.CustomType2 }
bind_java_type! { CustomType3 => com.example.CustomType3 }
bind_java_type! { CustomType4 => com.example.CustomType4 }
bind_java_type! { CustomType5 => com.example.CustomType5 }

bind_java_type! {
	rust_type = TestClass,
	java_type = com.example.TestClass,

	type_map {
		CustomType1 => com.example.CustomType1,
		CustomType2 => com.example.CustomType2,
	},

	type_map {
		CustomType3 => com.example.CustomType3,
		CustomType4 => com.example.CustomType4,
	},

	type_map {
		CustomType5 => com.example.CustomType5,
	},

	methods {
		fn test_method1(arg1: CustomType1) -> CustomType2,
		fn test_method2(arg2: CustomType2, arg3: CustomType3) -> CustomType4,
		fn test_method3(arg4: CustomType4) -> CustomType5,
	}
}

#[test]
fn test_bind_with_multiple_type_maps_compiles() {
	// This test just verifies compilation succeeds
}
