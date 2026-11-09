#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	objects::JThrowable,
	vm::{AttachConfig, AttachmentExceptionPolicy},
};
use rusty_fork::rusty_fork_test;

rusty_fork_test! {
#[test]
fn test_exception_unsafe_calls() {
	let jvm = util::jvm();
	let res = jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		assert!(matches!(env.throw("Test Exception".to_string()), Err(abos_jni::errors::Error::JavaException)));

		let res = env.new_string("Test");

		println!("new_string res = {res:?}");
		// No new string should be allocated because there is a pending exception
		// and until the exception is cleared, the JNI calls would lead to
		// undefined behavior
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));

		let pending = env.with_local_frame(8, |env| -> abos_jni::errors::Result<bool> {
			Ok(env.exception_check())
		}).expect("Push/PopLocalFrame + ExceptionCheck should be exception safe");

		assert!(pending);
		Ok(())
	});

	println!("res = {res:?}");
	assert_caught_exception(&res, "java.lang.RuntimeException", Some("Test Exception"));
}
}

#[test]
fn test_bind_java_type_exception_safety_checks() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));
		let res: std::result::Result<abos_jni::objects::JThread<'_>, abos_jni::errors::Error> =
			abos_jni::objects::JThread::current_thread(env);
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());

		env.exception_clear();
		let res: std::result::Result<abos_jni::objects::JThread<'_>, abos_jni::errors::Error> =
			abos_jni::objects::JThread::current_thread(env);
		assert!(res.is_ok());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_get_method_id_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let s: abos_jni::objects::JString<'_> = env.new_string("Test").expect("test string can be allocated");

		let str_class: abos_jni::objects::JClass<'_> = env.get_object_class(&s).expect("get object class succeeds");
		let res: std::result::Result<abos_jni::ids::JMethodID, abos_jni::errors::Error> = env.get_method_id(
			str_class,
			abos_jni::jni_str!("invalidMethod"),
			abos_jni::jni_sig!("()V"),
		);
		println!("method ID lookup result {res:?}");

		match res {
			Err(abos_jni::errors::Error::MethodNotFound { .. }) => {
				assert!(
					!env.exception_check(),
					"Expected no pending exception with MethodNotFound error"
				);
			}
			Err(abos_jni::errors::Error::JavaException) => {
				assert!(env.exception_check());
				panic!("Expected MethodNotFound error for invalid method lookup");
			}
			_ => {
				assert!(
					!env.exception_check(),
					"Spurious pending exception without JavaException error"
				);
				panic!("Expected a MethodNotFound error for invalid method lookup");
			}
		}
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_get_field_id_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let s: abos_jni::objects::JString<'_> = env.new_string("Test").expect("test string can be allocated");

		let str_class: abos_jni::objects::JClass<'_> = env.get_object_class(&s).expect("get object class succeeds");
		let res: std::result::Result<abos_jni::ids::JFieldID, abos_jni::errors::Error> =
			env.get_field_id(str_class, abos_jni::jni_str!("invalidField"), abos_jni::jni_sig!("Z"));
		println!("field ID lookup result {res:?}");

		match res {
			Err(abos_jni::errors::Error::FieldNotFound { .. }) => {
				assert!(
					!env.exception_check(),
					"Expected no pending exception with FieldNotFound error"
				);
			}
			Err(abos_jni::errors::Error::JavaException) => {
				assert!(env.exception_check());
				panic!("Expected FieldNotFound error for invalid field lookup");
			}
			_ => {
				assert!(
					!env.exception_check(),
					"Spurious pending exception without JavaException error"
				);
				panic!("Expected a FieldNotFound error for invalid field lookup");
			}
		}
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_find_class_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let res: std::result::Result<abos_jni::objects::JClass<'_>, abos_jni::errors::Error> =
			env.find_class(abos_jni::jni_str!("java/lang/string")); // typo of `String`
		println!("result = {res:?}");
		match res {
			Err(abos_jni::errors::Error::NoClassDefFound { .. }) => {
				assert!(
					!env.exception_check(),
					"Expected no pending exception with NoClassDefFound error"
				);
				println!("Got NoClassDefFound error as expected");
			}
			Err(abos_jni::errors::Error::JavaException) => {
				assert!(env.exception_check());
				env.exception_describe();
				panic!("Expected NoClassDefFound error for find_class failure");
			}
			_ => {
				assert!(
					!env.exception_check(),
					"Spurious pending exception without JavaException error"
				);
				panic!("Expected NoClassDefFound error for find_class failure");
			}
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_load_class_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let res: std::result::Result<abos_jni::objects::JClass<'_>, abos_jni::errors::Error> =
			env.load_class(abos_jni::jni_str!("java/lang/string")); // typo of `String`
		println!("result = {res:?}");
		match res {
			Err(abos_jni::errors::Error::NoClassDefFound { .. }) => {
				assert!(
					!env.exception_check(),
					"Expected no pending exception with NoClassDefFound error"
				);
				println!("Got NoClassDefFound error as expected");
			}
			Err(abos_jni::errors::Error::JavaException) => {
				assert!(env.exception_check());
				env.exception_describe();
				panic!("Expected NoClassDefFound error for load_class failure");
			}
			_ => {
				assert!(
					!env.exception_check(),
					"Spurious pending exception without JavaException error"
				);
				panic!("Expected NoClassDefFound error for load_class failure");
			}
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_global_drop_no_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let s: abos_jni::objects::JString<'_> = env.new_string("Test").expect("test string can be allocated");
		let global: abos_jni::refs::Global<abos_jni::objects::JString<'_>> =
			env.new_global_ref(s).expect("global reference can be allocated");

		// Don't use '?' here because we want to continue with the JavaException error
		// indicating a pending exception.
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));

		assert!(env.exception_check());

		drop(global);

		assert!(
			env.exception_check(),
			"Expected exception to still be pending after dropping global reference"
		);

		env.exception_clear();

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_weak_drop_no_exception_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let s: abos_jni::objects::JString<'_> = env.new_string("Test").expect("test string can be allocated");
		let weak: abos_jni::refs::Weak<abos_jni::objects::JString<'_>> =
			env.new_weak_ref(s).expect("weak reference can be allocated");

		// Don't use '?' here because we want to continue with the JavaException error
		// indicating a pending exception.
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));

		assert!(env.exception_check());

		drop(weak);
		assert!(
			env.exception_check(),
			"Expected exception to still be pending after dropping weak reference"
		);

		env.exception_clear();

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_env_throw_apis_return_java_exception_err() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let res: std::result::Result<(), abos_jni::errors::Error> = env.throw("Test Exception");
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());

		let catch: std::result::Result<(), abos_jni::errors::Error> = env.exception_catch();
		println!("Caught exception: {catch:?}");
		assert_caught_exception(&catch, "java.lang.RuntimeException", Some("Test Exception"));
		assert!(!env.exception_check());

		let res: std::result::Result<(), abos_jni::errors::Error> = env.throw_new(
			abos_jni::jni_str!("java/lang/Exception"),
			abos_jni::jni_str!("something bad happened"),
		);
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());

		let catch: std::result::Result<(), abos_jni::errors::Error> = env.exception_catch();
		println!("Caught exception: {catch:?}");
		assert_caught_exception(&catch, "java.lang.Exception", Some("something bad happened"));
		assert!(!env.exception_check());

		let res: std::result::Result<(), abos_jni::errors::Error> =
			env.throw_new_void(abos_jni::jni_str!("java/lang/NullPointerException"));
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());

		let catch: std::result::Result<(), abos_jni::errors::Error> = env.exception_catch();
		println!("Caught exception: {catch:?}");
		assert_caught_exception(&catch, "java.lang.NullPointerException", None);
		assert!(!env.exception_check());

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn test_no_attach_current_thread_side_effects() {
	let jvm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();

	jvm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let jvm: abos_jni::JavaVM = abos_jni::JavaVM::singleton().expect("test JVM has been initialized");

		// Don't use '?' here because we want to continue with the JavaException error
		// indicating a pending exception.
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));

		let res: std::result::Result<(), abos_jni::errors::Error> = jvm.attach_current_thread(|env| {
			let msg: abos_jni::objects::JString<'_> = env.new_string("Bad Format")?;
			let e: abos_jni::exceptions::JNumberFormatException<'_> =
				abos_jni::exceptions::JNumberFormatException::new(env, msg)?;
			let e: JThrowable = e.into();
			env.throw(e)
		});
		println!("attach_current_thread_result = {res:?}");
		// The test exception should have been temporarily cleared before running the
		// nested attachment closure, and we should get back the exception that the
		// closure throws
		assert_caught_exception(&res, "java.lang.NumberFormatException", Some("Bad Format"));

		// The original 'Test Exception' should be re-thrown and still be pending here
		let catch: std::result::Result<(), abos_jni::errors::Error> = env.exception_catch();
		println!("Caught exception: {catch:?}");
		assert_caught_exception(&catch, "java.lang.RuntimeException", Some("Test Exception"));

		// For reference, we also check the behaviour of
		// `AttachmentExceptionPolicy::PreCheckPostCatch` which was the
		// behaviour that `attach_current_thread` (briefly) had in 0.22.1

		// Don't use '?' here because we want to continue with the JavaException error
		// indicating a pending exception.
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));

		let res: std::result::Result<(), abos_jni::errors::Error> = jvm.attach_current_thread_with_config(
			|| AttachConfig::default().exceptions_policy(AttachmentExceptionPolicy::PreCheckPostCatch),
			Some(8),
			|_env| -> abos_jni::errors::Result<()> {
				panic!("Shouldn't run closure while exception is pending");
			},
		);
		assert!(matches!(res, Err(abos_jni::errors::Error::JavaException)));

		let catch: std::result::Result<(), abos_jni::errors::Error> = env.exception_catch();
		println!("Caught exception: {catch:?}");
		assert_caught_exception(&catch, "java.lang.RuntimeException", Some("Test Exception"));

		// Finally we check the behaviour of `AttachmentExceptionPolicy::Ignore`
		assert!(matches!(
			env.throw("Test Exception"),
			Err(abos_jni::errors::Error::JavaException)
		));
		let pending: bool = jvm
			.attach_current_thread_with_config(
				|| AttachConfig::default().exceptions_policy(AttachmentExceptionPolicy::Ignore),
				Some(8),
				|env| -> abos_jni::errors::Result<_> { Ok(env.exception_check()) },
			)
			.expect("Failed to attach current thread with config and clear exception");

		// The callback should have been able to observe the pending exception
		assert!(pending);

		// And attach_current_thread_with_config should have left the pending exception un-touched
		assert!(
			env.exception_check(),
			"Ignore policy should leave pending exception untouched"
		);
		env.exception_clear();

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn assert_caught_exception<T: std::fmt::Debug>(
	result: &abos_jni::errors::Result<T>,
	expected_name: &str,
	expected_message: Option<&str>,
) {
	let Err(abos_jni::errors::Error::CaughtJavaException { name, msg, .. }) = result else {
		panic!("Expected a caught Java exception, got {result:?}");
	};
	assert_eq!(name, expected_name);
	if let Some(message) = expected_message {
		assert_eq!(msg, message);
	}
}

struct ExceptionOnAsRef<'a, 'local, T> {
	env: &'a abos_jni::Env<'local>,
	exception_class: &'a abos_jni::objects::JClass<'local>,
	value: &'a T,
	calls: std::cell::Cell<usize>,
}

impl<T> AsRef<T> for ExceptionOnAsRef<'_, '_, T> {
	fn as_ref(&self) -> &T {
		self.calls.set(self.calls.get() + 1);
		assert!(matches!(
			self.env
				.throw_new_from_class(self.exception_class, abos_jni::jni_str!("argument conversion")),
			Err(abos_jni::errors::Error::JavaException)
		));
		self.value
	}
}

#[test]
fn test_array_set_checks_exception_after_conversion() {
	util::attach_current_thread(|env| {
		let exception_class: abos_jni::objects::JClass<'_> =
			env.find_class(abos_jni::jni_str!("java/lang/RuntimeException"))?;
		let original: abos_jni::objects::JString<'_> = env.new_string("original")?;
		let replacement: abos_jni::objects::JString<'_> = env.new_string("replacement")?;
		let array: abos_jni::objects::JObjectArray<'_, abos_jni::objects::JString<'_>> =
			abos_jni::objects::JObjectArray::<abos_jni::objects::JString>::new(env, 1, &original)?;
		let value: ExceptionOnAsRef<'_, '_, abos_jni::objects::JString<'_>> = ExceptionOnAsRef {
			env,
			exception_class: &exception_class,
			value: &replacement,
			calls: std::cell::Cell::new(0),
		};

		let result: abos_jni::errors::Result<()> = array.set_element(env, 0, &value);
		assert_eq!(value.calls.get(), 1);
		assert!(matches!(result, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());
		assert_caught_exception(
			&env.exception_catch(),
			"java.lang.RuntimeException",
			Some("argument conversion"),
		);
		let element: abos_jni::objects::JString<'_> = array.get_element(env, 0)?;
		assert!(env.is_same_object(&element, &original)?);
		Ok(())
	})
	.expect("argument conversion must not modify the array or consume its exception");
}

#[test]
fn test_instance_check_checks_exception_after_conversion() {
	util::attach_current_thread(|env| {
		let exception_class: abos_jni::objects::JClass<'_> =
			env.find_class(abos_jni::jni_str!("java/lang/RuntimeException"))?;
		let string_class: abos_jni::objects::JClass<'_> = env.find_class(abos_jni::jni_str!("java/lang/String"))?;
		let object: abos_jni::objects::JObject<'_> = env.new_string("value")?.into();
		let value: ExceptionOnAsRef<'_, '_, abos_jni::objects::JObject<'_>> = ExceptionOnAsRef {
			env,
			exception_class: &exception_class,
			value: &object,
			calls: std::cell::Cell::new(0),
		};

		let result: abos_jni::errors::Result<bool> = env.is_instance_of_class(&value, &string_class);
		assert_eq!(value.calls.get(), 1);
		assert!(matches!(result, Err(abos_jni::errors::Error::JavaException)));
		assert!(env.exception_check());
		assert_caught_exception(
			&env.exception_catch(),
			"java.lang.RuntimeException",
			Some("argument conversion"),
		);
		Ok(())
	})
	.expect("instance checks must preserve conversion exceptions");
}

#[test]
fn test_nested_attachment_panic_restores_pending_exception() {
	util::attach_current_thread(|env| {
		for capacity in [None, Some(8)] {
			for throw_in_callback in [false, true] {
				assert_panic_preserves_exception(env, capacity, throw_in_callback)?;
			}
		}
		Ok(())
	})
	.expect("nested attachment restores the original exception before resuming a panic");
}

fn assert_panic_preserves_exception(
	env: &mut abos_jni::Env<'_>,
	capacity: Option<usize>,
	throw_in_callback: bool,
) -> abos_jni::errors::Result<()> {
	let vm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();
	assert!(matches!(
		env.throw("original failure"),
		Err(abos_jni::errors::Error::JavaException)
	));
	let original: JThrowable<'_> = env.exception_occurred().expect("original exception is pending");
	let level: usize = abos_jni::JavaVM::thread_attach_guard_level();
	let payload: std::sync::Arc<()> = std::sync::Arc::new(());

	let unwind: std::thread::Result<abos_jni::errors::Result<()>> =
		std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			vm.attach_current_thread_with_config(AttachConfig::default, capacity, |env| {
				assert!(!env.exception_check(), "the callback starts with no pending exception");
				if throw_in_callback {
					assert!(matches!(
						env.throw("callback failure"),
						Err(abos_jni::errors::Error::JavaException)
					));
				}
				std::panic::panic_any(std::sync::Arc::clone(&payload));
			})
		}));
	let caught: Box<dyn std::any::Any + Send> = unwind.expect_err("callback panic must propagate");
	assert!(std::sync::Arc::ptr_eq(
		caught
			.downcast_ref::<std::sync::Arc<()>>()
			.expect("original panic payload type"),
		&payload,
	));
	assert_eq!(abos_jni::JavaVM::thread_attach_guard_level(), level);
	let restored: JThrowable<'_> = env
		.exception_occurred()
		.expect("original exception is restored after panic");
	env.exception_clear();
	assert!(env.is_same_object(&original, &restored)?);
	Ok(())
}

#[test]
fn test_scoped_attachment_panic_detaches() {
	let vm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();
	std::thread::spawn(move || {
		assert!(!vm.is_thread_attached().expect("worker starts detached"));
		let unwind: std::thread::Result<abos_jni::errors::Result<()>> = std::panic::catch_unwind(|| {
			vm.attach_current_thread_for_scope(|_env| {
				panic!("scoped callback panic");
			})
		});
		assert!(unwind.is_err());
		assert!(!vm.is_thread_attached().expect("worker is detached after panic"));
		assert_eq!(abos_jni::JavaVM::thread_attach_guard_level(), 0);
		vm.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
			assert!(!env.exception_check());
			let _ = env.new_string("attachment remains usable")?;
			Ok(())
		})
		.expect("worker can reattach after a panic");
	})
	.join()
	.expect("scoped attachment cleanup survives a callback panic");
}
