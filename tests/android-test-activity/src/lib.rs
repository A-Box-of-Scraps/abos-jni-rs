//! TestActivity JNI implementation
use test_error::{TestError, TestResult};

mod iterators;
mod strings;
mod test_error;

pub mod sdk;

// Include the generated bindings from build.rs
include!(concat!(env!("OUT_DIR"), "/test_activity_bindings.rs"));

use io::github::jni_rs::jbindgen::testactivity::{TestActivity, TestActivityAPI, TestActivityNativeInterface};

struct NativeTest {
	name: &'static str,
	runner: for<'local> fn(&mut abos_jni::Env<'local>) -> TestResult,
}

const NATIVE_TESTS: &[NativeTest] = &[
	NativeTest {
		name: "jni_array_map_iterator",
		runner: iterators::test_array_map,
	},
	#[cfg(feature = "sdk_util_time_utils")]
	NativeTest {
		name: "sdk_util_time_utils",
		runner: sdk::util_time_utils::test_time_utils,
	},
	#[cfg(feature = "sdk_os_build")]
	NativeTest {
		name: "sdk_os_build",
		runner: sdk::os_build::test_os_build,
	},
	#[cfg(feature = "sdk_os_binder")]
	NativeTest {
		name: "sdk_os_binder_resolution",
		runner: sdk::os_binder::test_os_binder,
	},
	#[cfg(feature = "sdk_bluetooth")]
	NativeTest {
		name: "sdk_bluetooth_resolution",
		runner: sdk::bluetooth::test_bluetooth,
	},
	#[cfg(feature = "sdk_content_intent")]
	NativeTest {
		name: "sdk_content_intent",
		runner: sdk::content_intent::test_content_intent,
	},
	#[cfg(feature = "sdk_net_uri")]
	NativeTest {
		name: "sdk_net_uri",
		runner: sdk::net_uri::test_net_uri,
	},
];

impl TestActivityNativeInterface for TestActivityAPI {
	type Error = TestError;

	fn native_on_create<'local>(
		env: &mut abos_jni::Env<'local>,
		this: TestActivity<'local>,
	) -> Result<(), Self::Error> {
		let message = abos_jni::objects::JString::from_str(env, "Hello from Rust native onCreate!")?;
		this.update_ui(env, message)?;

		Ok(())
	}

	fn native_get_message<'local>(
		env: &mut abos_jni::Env<'local>,
		_this: TestActivity<'local>,
	) -> Result<abos_jni::objects::JString<'local>, Self::Error> {
		strings::test_roundtrips(env)?;
		Ok(abos_jni::objects::JString::from_str(env, "Message from Rust!")?)
	}

	fn native_process_data<'local>(
		_env: &mut abos_jni::Env<'local>,
		_this: TestActivity<'local>,
		value: abos_jni::sys::jint,
	) -> Result<abos_jni::sys::jint, Self::Error> {
		Ok(value * 2)
	}

	fn native_get_test_count<'local>(
		_env: &mut abos_jni::Env<'local>,
		_this: TestActivity<'local>,
	) -> Result<abos_jni::sys::jint, Self::Error> {
		Ok(i32::try_from(NATIVE_TESTS.len()).expect("test registry fits in a Java int"))
	}

	fn native_get_test_name<'local>(
		env: &mut abos_jni::Env<'local>,
		_this: TestActivity<'local>,
		index: abos_jni::sys::jint,
	) -> Result<abos_jni::objects::JString<'local>, Self::Error> {
		Ok(abos_jni::objects::JString::from_str(env, test_at(index)?.name)?)
	}

	fn native_run_test<'local>(
		env: &mut abos_jni::Env<'local>,
		_this: TestActivity<'local>,
		index: abos_jni::sys::jint,
	) -> Result<abos_jni::objects::JString<'local>, Self::Error> {
		let message = (test_at(index)?.runner)(env)?;
		Ok(abos_jni::objects::JString::from_str(env, message)?)
	}
}

fn test_at(index: i32) -> Result<&'static NativeTest, TestError> {
	usize::try_from(index)
		.ok()
		.and_then(|index| NATIVE_TESTS.get(index))
		.ok_or_else(|| TestError::Assertion(format!("invalid native test index: {index}")))
}

#[cfg(test)]
mod tests {
	use super::{NATIVE_TESTS, test_at};

	#[test]
	fn rejects_invalid_indices_without_advancing_global_state() {
		assert!(test_at(-1).is_err());
		assert!(test_at(i32::try_from(NATIVE_TESTS.len()).expect("small registry")).is_err());
		for (index, test) in NATIVE_TESTS.iter().enumerate() {
			let index = i32::try_from(index).expect("small registry");
			assert_eq!(test_at(index).expect("valid index").name, test.name);
			assert_eq!(test_at(index).expect("same index remains valid").name, test.name);
		}
	}
}
