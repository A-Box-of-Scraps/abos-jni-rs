#![allow(unsafe_code, reason = "Binding macros generate JNI calls and native wrappers.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use abos_jni::{
	Env, bind_java_type,
	errors::{Error, Result},
	jni_str,
	objects::{JClass, JObject},
	refs::LoaderContext,
};
use rusty_fork::rusty_fork_test;

static NATIVE_REQUIREMENT_CALLS: AtomicUsize = AtomicUsize::new(0);
static FAIL_REQUIREMENT: AtomicBool = AtomicBool::new(true);

fn native_value(env: &Env<'_>, _class: JClass<'_>, value: i32) -> Result<i32> {
	if env.exception_check() {
		return Err(Error::JavaException);
	}
	Ok(value + 1)
}

fn native_instance_value(env: &Env<'_>, _this: OptionalBindings<'_>, value: i32) -> Result<i32> {
	if env.exception_check() {
		return Err(Error::JavaException);
	}
	Ok(value - 1)
}

fn check(env: &mut Env<'_>) -> Result<bool> {
	let class: JClass<'_> = env.find_class(jni_str!("java/lang/Object"))?;
	Ok(class.is_null())
}

bind_java_type! {
	OptionalBindings => "com.example.TestOptionalBindings",
	constructors {
		fn new(),
		fn with_value { sig = (value: jint), requires = available },
		fn missing_constructor { sig = (value: JObject), requires = available },
		fn disabled_constructor { sig = (), requires = |_| Ok(false) },
	},
	methods {
		fn present() -> jint,
		fn optional_present { name = "present", sig = () -> jint, requires = available },
		fn missing { sig = (value: JObject) -> JObject, requires = available },
		static fn static_present { sig = () -> jint, requires = available },
		static fn static_missing { sig = () -> jint, requires = available },
		fn disabled { name = "present", sig = () -> jint, requires = |_| Ok(false) },
		fn disabled_missing { sig = () -> jint, requires = check },
		static fn disabled_native_is_unregistered() -> bool,
		#[cfg(any())]
		fn excluded { sig = (), requires = nonexistent_predicate },
	},
	fields {
		value { sig = jint, requires = available },
		object { sig = JObject, requires = available },
		missing_field { sig = JObject, requires = available },
		static static_value { sig = jint, requires = available },
		static static_object { sig = JObject, requires = available },
		static missing_static_field { sig = jint, requires = available },
		disabled_field { name = "value", sig = jint, requires = |_| Ok(false) },
		#[cfg(any())]
		excluded_field { sig = jint, requires = nonexistent_predicate },
	},
	native_methods {
		pub fn instance_native { sig = (value: jint) -> jint, fn = native_instance_value, requires = available },
		pub fn missing_instance_native { sig = (value: jint) -> jint, fn = native_instance_value, requires = available },
		static fn required_native { sig = (value: jint) -> jint, fn = native_value },
		pub static fn optional_native { sig = (value: jint) -> jint, fn = native_value, requires = available },
		pub static fn missing_native { sig = (value: jint) -> jint, fn = native_value, requires = available },
		pub static fn guarded_native {
			sig = (value: jint) -> jint,
			fn = native_value,
			requires = |_| {
				NATIVE_REQUIREMENT_CALLS.fetch_add(1, Ordering::SeqCst);
				Ok(true)
			},
		},
		pub static fn disabled_native { sig = (value: jint) -> jint, fn = native_value, requires = |_| Ok(false) },
		#[cfg(any())]
		pub static fn excluded_native { sig = (value: jint) -> jint, fn = native_value, requires = nonexistent_predicate },
	}
}

bind_java_type! {
	OptionalOnlyNatives => "java.util.ArrayList",
	native_methods {
		pub static fn missing { sig = (value: jint) -> jint, fn = native_value, requires = available },
	}
}

bind_java_type! {
	RequiredMissing => "java.util.ArrayList",
	methods { fn missing() -> jint },
}

bind_java_type! {
	EnabledMissing => "java.util.ArrayList",
	methods { fn missing { sig = () -> jint, requires = |_| Ok(true) } },
}

bind_java_type! {
	RetryRequirement => "java.util.ArrayList",
	constructors { fn new() },
	methods {
		fn size {
			sig = () -> jint,
			requires = |_| {
				if FAIL_REQUIREMENT.swap(false, Ordering::SeqCst) {
					Err(Error::WrongObjectType)
				} else {
					Ok(true)
				}
			},
		},
	}
}

bind_java_type! {
	ThrowingRequirement => "java.util.ArrayList",
	constructors { fn new() },
	methods {
		fn size {
			sig = () -> jint,
			requires = |env| {
				assert!(matches!(
					env.throw_new(jni_str!("java/lang/IllegalStateException"), jni_str!("requirement failed")),
					Err(Error::JavaException)
				));
				Ok(false)
			},
		},
	}
}

bind_java_type! {
	InitializerFailure => "com.example.TestOptionalInitializerFailure",
	methods { static fn present { sig = () -> jint, requires = available } },
}

bind_java_type! {
	MethodInitializerFailure => "com.example.TestOptionalMethodInitializerFailure",
	methods { static fn present { sig = () -> jint, requires = available } },
}

bind_java_type! {
	InstanceMethodInitializerFailure => "com.example.TestOptionalMethodInitializerFailure",
	methods { fn instance_present { sig = () -> jint, requires = available } },
}

bind_java_type! {
	ConstructorInitializerFailure => "com.example.TestOptionalMethodInitializerFailure",
	constructors { fn new { sig = (), requires = available } },
}

bind_java_type! {
	FieldInitializerFailure => "com.example.TestOptionalFieldInitializerFailure",
	fields { static present { sig = jint, requires = available } },
}

bind_java_type! {
	InstanceFieldInitializerFailure => "com.example.TestOptionalFieldInitializerFailure",
	fields { instance_present { sig = jint, requires = available } },
}

fn assert_initializer_failure_not_cached(class_name: &str, mut initialize: impl FnMut(&mut Env<'_>) -> Result<()>) {
	util::attach_current_thread(|env| {
		util::load_test_class(env, &util::java_fixtures(), class_name)?;
		// A failed initializer must fail API initialization, including on subsequent attempts.
		for _ in 0..2 {
			let error: Error = initialize(env).expect_err("initializer failure must not initialize the binding API");
			assert!(!matches!(
				error,
				Error::MemberUnavailable { .. } | Error::MethodNotFound { .. } | Error::FieldNotFound { .. }
			));
			assert!(!env.exception_check());
		}
		Ok(())
	})
	.expect("initializer linkage errors are not cached as absent members");
}

fn assert_unavailable<T: std::fmt::Debug>(result: Result<T>, name: &str) {
	let error: Error = result.expect_err("member must be unavailable");
	assert!(matches!(error, Error::MemberUnavailable { name: member } if member == name));
}

fn load_optional_bindings(env: &mut Env<'_>) -> Result<()> {
	util::load_test_class(env, &util::java_fixtures(), "TestOptionalBindings")
}

rusty_fork_test! {
	#[test]
	fn missing_members_do_not_poison_available_members() {
		util::attach_current_thread(|env| {
			load_optional_bindings(env)?;
			let object: OptionalBindings<'_> = OptionalBindings::new(env)?;
			assert_eq!(object.present(env)?, 7);
			assert_eq!(object.optional_present(env)?, 7);
			assert_eq!(OptionalBindings::with_value(env, 19)?.value(env)?, 19);
			assert_eq!(OptionalBindings::static_present(env)?, 36);
			assert_unavailable(OptionalBindings::missing_constructor(env, JObject::null()), "<init>");
			assert_unavailable(OptionalBindings::disabled_constructor(env), "<init>");
			assert_unavailable(object.missing(env, JObject::null()), "missing");
			assert_unavailable(OptionalBindings::static_missing(env), "staticMissing");
			assert_unavailable(object.disabled(env), "present");
			assert_unavailable(object.disabled_missing(env), "disabledMissing");
			assert_unavailable(object.missing_field(env), "missingField");
			assert_unavailable(object.set_missing_field(env, JObject::null()), "missingField");
			assert_unavailable(OptionalBindings::missing_static_field(env), "missingStaticField");
			assert_unavailable(OptionalBindings::set_missing_static_field(env, 1), "missingStaticField");
			assert_unavailable(object.disabled_field(env), "value");
			assert_unavailable(object.set_disabled_field(env, 1), "value");
			assert!(!env.exception_check());

			object.set_value(env, 42)?;
			assert_eq!(object.present(env)?, 42);
			object.set_object(env, &object)?;
			let value: JObject<'_> = object.object(env)?;
			assert!(env.is_same_object(value, &object)?);
			OptionalBindings::set_static_object(env, &object)?;
			let value: JObject<'_> = OptionalBindings::static_object(env)?;
			assert!(env.is_same_object(value, &object)?);
			OptionalBindings::set_static_value(env, 99)?;
			assert_eq!(OptionalBindings::static_present(env)?, 99);

			Ok(())
		}).expect("available members remain usable");
	}

	#[test]
	fn cached_unavailable_calls_preserve_pending_exceptions() {
		util::attach_current_thread(|env| {
			load_optional_bindings(env)?;
			let object: OptionalBindings<'_> = OptionalBindings::new(env)?;
			// Even cached unavailable calls must preserve an existing exception.
			assert!(matches!(
				env.throw_new(jni_str!("java/lang/IllegalStateException"), jni_str!("already pending")),
				Err(Error::JavaException)
			));
			assert!(matches!(object.missing(env, JObject::null()), Err(Error::JavaException)));
			assert!(matches!(OptionalBindings::disabled_constructor(env), Err(Error::JavaException)));
			assert!(matches!(object.missing_field(env), Err(Error::JavaException)));
			assert!(matches!(object.set_missing_field(env, JObject::null()), Err(Error::JavaException)));
			assert!(matches!(OptionalBindings::missing_native(env, 1), Err(Error::JavaException)));
			assert!(env.exception_check());
			env.exception_clear();
			assert_eq!(object.present(env)?, 7);
			Ok(())
		}).expect("cached unavailable members preserve pending exceptions");
	}

	#[test]
	fn native_gates_preserve_registration_before_class_initialization() {
		util::attach_current_thread(|env| {
			load_optional_bindings(env)?;
			// Initializing the class invokes required, optional and predicate-gated natives.
			assert_eq!(OptionalBindings::static_present(env)?, 36);
			assert_eq!(OptionalBindings::optional_native(env, 40)?, 41);
			assert_eq!(OptionalBindings::guarded_native(env, 41)?, 42);
			let object: OptionalBindings<'_> = OptionalBindings::new(env)?;
			assert_eq!(object.instance_native(env, 43)?, 42);
			assert_unavailable(object.missing_instance_native(env, 1), "missingInstanceNative");
			assert_eq!(NATIVE_REQUIREMENT_CALLS.load(Ordering::SeqCst), 1);
			assert_unavailable(OptionalBindings::missing_native(env, 1), "missingNative");
			assert_unavailable(OptionalBindings::disabled_native(env, 1), "disabledNative");
			assert!(OptionalBindings::disabled_native_is_unregistered(env)?);
			assert_unavailable(OptionalOnlyNatives::missing(env, 1), "missing");
			assert!(!env.exception_check());
			Ok(())
		}).expect("only enabled natives are registered");
	}

	#[test]
	fn enabled_and_required_missing_members_still_fail_initialization() {
		util::attach_current_thread(|env| {
			assert!(matches!(RequiredMissingAPI::get(env, &LoaderContext::None), Err(Error::MethodNotFound { .. })));
			assert!(!env.exception_check());
			assert!(matches!(EnabledMissingAPI::get(env, &LoaderContext::None), Err(Error::MethodNotFound { .. })));
			assert!(!env.exception_check());
			Ok(())
		}).expect("requirements do not hide lookup failures for enabled members");
	}

	#[test]
	fn predicate_errors_propagate_and_initialization_can_retry() {
		util::attach_current_thread(|env| {
			assert!(matches!(RetryRequirement::new(env), Err(Error::WrongObjectType)));
			assert!(!env.exception_check());
			let object: RetryRequirement<'_> = RetryRequirement::new(env)?;
			assert_eq!(object.size(env)?, 0);
			Ok(())
		}).expect("predicate errors do not cache a partially initialized API");
	}

	#[test]
	fn pending_exception_from_predicate_is_not_an_unavailable_member() {
		util::attach_current_thread(|env| {
			assert!(matches!(ThrowingRequirement::new(env), Err(Error::JavaException)));
			assert!(env.exception_check());
			env.exception_clear();
			Ok(())
		}).expect("predicate exceptions stay pending");
	}

	#[test]
	fn optional_lookup_does_not_hide_class_initialization_failure() {
		util::attach_current_thread(|env| {
			util::load_test_class(env, &util::java_fixtures(), "TestOptionalInitializerFailure")?;
			assert!(matches!(InitializerFailure::present(env), Err(Error::ExceptionInInitializer { .. })));
			assert!(!env.exception_check());
			Ok(())
		}).expect("initializer failure is not treated as absence");
	}

	#[test]
	fn optional_static_method_does_not_hide_initializer_linkage_error() {
		assert_initializer_failure_not_cached("TestOptionalMethodInitializerFailure", |env| {
			MethodInitializerFailure::present(env).map(|_| ())
		});
	}

	#[test]
	fn optional_instance_method_does_not_hide_initializer_linkage_error() {
		assert_initializer_failure_not_cached("TestOptionalMethodInitializerFailure", |env| {
			InstanceMethodInitializerFailureAPI::get(env, &LoaderContext::None).map(|_| ())
		});
	}

	#[test]
	fn optional_constructor_does_not_hide_initializer_linkage_error() {
		assert_initializer_failure_not_cached("TestOptionalMethodInitializerFailure", |env| {
			ConstructorInitializerFailure::new(env).map(|_| ())
		});
	}

	#[test]
	fn optional_static_field_does_not_hide_initializer_linkage_error() {
		assert_initializer_failure_not_cached("TestOptionalFieldInitializerFailure", |env| {
			FieldInitializerFailure::present(env).map(|_| ())
		});
	}

	#[test]
	fn optional_instance_field_does_not_hide_initializer_linkage_error() {
		assert_initializer_failure_not_cached("TestOptionalFieldInitializerFailure", |env| {
			InstanceFieldInitializerFailureAPI::get(env, &LoaderContext::None).map(|_| ())
		});
	}
}
