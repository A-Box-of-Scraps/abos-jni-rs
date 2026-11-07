#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
mod inner {
	use abos_jni_macros::bind_java_type;

	bind_java_type! {
		pub TestVisibility => "com.example.TestVisibility",
		methods {
			fn public_method() -> void,

			pub(self) fn private_method() -> void,

			priv fn another_private() -> void,

			pub(crate) fn crate_method() -> void,
		}
	}

	bind_java_type! {
		rust_type = TestVisibilityBlock,
		java_type = "com.example.TestVisibilityBlock",
		methods {
			fn block_public {
				sig = () -> void,
			},

			pub(self) fn block_private {
				sig = () -> void,
			},

			priv fn another_block_private {
				sig = () -> void,
			},
		}
	}

	bind_java_type! {
		rust_type = TestFieldVisibility,
		java_type = "com.example.TestFieldVisibility",
		fields {
			public_field: int,

			pub(self) private_field: int,

			priv another_private_field: int,

			pub(crate) crate_field: int,
		}
	}

	bind_java_type! {
		rust_type = TestFieldVisibilityIndependent,
		java_type = "com.example.TestFieldVisibilityIndependent",
		fields {
			mixed_visibility {
				sig = int,
				pub get = get_mixed,
				pub(self) set = set_mixed,
			},

			both_private {
				sig = int,
				priv get = get_private,
				priv set = set_private,
			},
		}
	}

	bind_java_type! {
		rust_type = TestFieldVisibilityInherited,
		java_type = "com.example.TestFieldVisibilityInherited",
		fields {
			pub(self) inherited_field: int,

			pub(self) override_setter {
				sig = int,
				pub set = set_override,
			},
		}
	}

	#[test]
	fn test_private_methods_exist() {
		let _ = TestVisibility::private_method;
	}
}

#[test]
fn test_public_methods_exist() {
	let _ = inner::TestVisibility::public_method;
}
