#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
//! Shows how the `load_class` and `init_priv` hooks can be used

use abos_jni_macros::bind_java_type;

#[derive(Debug)]
struct JTest2Priv {
	#[allow(
		unused,
		reason = "The example demonstrates initialization of private state without reading it back."
	)]
	initialized: bool,
	// make it something that's not automatically Send/Sync
	_need_send_sync: std::marker::PhantomData<*mut ()>,
}
// SAFETY: The state contains only a bool and PhantomData, with no actual thread-affine pointer to transfer.
unsafe impl Send for JTest2Priv {}
// SAFETY: The state contains only a bool and PhantomData, with no mutable or thread-affine pointee to share.
unsafe impl Sync for JTest2Priv {}

bind_java_type! {
	rust_type = JTest2,
	java_type = "com.example.Test2",
	is_instance_of = { JString },

	priv_type = JTest2Priv,

	hooks {
		load_class = |env, load_context, initialize| {
			load_context.load_class_for_type::<JTest2>(env, initialize)
		},
		init_priv = |_env, _class, _load_context| {
			Ok(JTest2Priv { initialized: true, _need_send_sync: std::marker::PhantomData })
		},
	}
}

fn main() {
	println!("Compiled successfully!");
}
