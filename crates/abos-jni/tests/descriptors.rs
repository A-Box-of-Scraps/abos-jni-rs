#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;
use util::attach_current_thread;

use abos_jni::{
	descriptors::Desc,
	jni_str,
	objects::{Auto, JClass},
};

#[test]
fn test_descriptors() {
	attach_current_thread(|env| {
		let class_local: JClass<'_> = env
			.find_class(jni_str!("java/lang/String"))
			.expect("test class can be resolved");
		let class_as_ref: &JClass<'_> = Desc::<JClass>::lookup(&class_local, env).expect("lookup succeeds");
		let class_global: abos_jni::refs::Global<JClass<'_>> = env
			.new_global_ref(class_as_ref)
			.expect("global reference can be allocated");
		let _class_as_ref: &JClass<'_> = Desc::<JClass>::lookup(&class_global, env).expect("lookup succeeds");
		let class_auto: Auto<'_, JClass<'_>> =
			Desc::<JClass>::lookup(jni_str!("java/lang/String"), env).expect("lookup succeeds");
		let _class_as_ref: &Auto<'_, JClass<'_>> = Desc::<JClass>::lookup(&class_auto, env).expect("lookup succeeds");

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
