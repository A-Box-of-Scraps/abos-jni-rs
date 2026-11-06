#![allow(unsafe_code, reason = "Shared JVM test helpers include raw JNI operations.")]
#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use std::panic::{AssertUnwindSafe, catch_unwind};

use abos_jni::{
	Env, MonitorGuard,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JObject, JThrowable, JValue},
	refs::{Auto, Global, IntoAuto as _},
};

fn holds_lock(env: &mut Env, object: &JObject) -> Result<bool> {
	env.call_static_method(
		jni_str!("java/lang/Thread"),
		jni_str!("holdsLock"),
		jni_sig!("(Ljava/lang/Object;)Z"),
		&[JValue::Object(object)],
	)?
	.z()
}

#[test]
fn monitor_is_reentrant_and_releases_on_drop() {
	util::attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		assert!(!holds_lock(env, &object)?);
		let guard: MonitorGuard<'_> = env.lock_obj(&object)?;
		assert!(holds_lock(env, &object)?);
		let nested: MonitorGuard<'_> = env.lock_obj(&object)?;
		drop(nested);
		assert!(holds_lock(env, &object)?);
		drop(guard);
		assert!(!holds_lock(env, &object)?);
		env.delete_local_ref(object);
		Ok(())
	})
	.expect("local monitor releases both acquisitions");
}

#[test]
fn monitor_borrows_auto_and_global_owners() {
	util::attach_current_thread(|env| {
		let object: Auto<'_, JObject<'_>> = env
			.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?
			.auto();
		let global: Global<JObject<'static>> = env.new_global_ref(&object)?;
		let guard: MonitorGuard<'_> = env.lock_obj(&object)?;
		assert!(holds_lock(env, global.as_ref())?);
		drop(guard);
		drop(object);
		assert!(!holds_lock(env, global.as_ref())?);

		let guard: MonitorGuard<'_> = env.lock_obj(&global)?;
		assert!(holds_lock(env, global.as_ref())?);
		drop(guard);
		assert!(!holds_lock(env, global.as_ref())?);
		drop(global);
		Ok(())
	})
	.expect("reference owners can be dropped after their guards");
}

#[test]
fn outer_reference_monitor_can_outlive_an_inner_frame() {
	util::attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let guard: MonitorGuard<'_> = env.with_local_frame(1, |inner| -> Result<_> {
			let guard: MonitorGuard<'_> = env.lock_obj(&object)?;
			assert!(holds_lock(inner, &object)?);
			Ok(guard)
		})?;
		assert!(holds_lock(env, &object)?);
		drop(guard);
		assert!(!holds_lock(env, &object)?);
		Ok(())
	})
	.expect("monitor borrowing an outer reference survives a nested frame");
}

#[test]
fn monitor_releases_during_unwinding() {
	util::attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let panic: std::thread::Result<()> = catch_unwind(AssertUnwindSafe(|| {
			let _guard: MonitorGuard<'_> = env.lock_obj(&object).expect("acquire monitor before unwinding");
			assert!(holds_lock(env, &object).expect("query held monitor"));
			panic!("unwind while holding a Java monitor");
		}));
		assert!(panic.is_err());
		assert!(!holds_lock(env, &object)?);
		Ok(())
	})
	.expect("unwinding releases the monitor");
}

#[test]
fn monitor_release_preserves_pending_exception() {
	util::attach_current_thread(|env| {
		let object: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let exception: JObject<'_> = env.new_object(jni_str!("java/lang/RuntimeException"), jni_sig!("()V"), &[])?;
		let exception: JThrowable<'_> = env.cast_local::<JThrowable>(exception)?;
		let guard: MonitorGuard<'_> = env.lock_obj(&object)?;
		assert!(matches!(env.throw(&exception), Err(Error::JavaException)));
		assert!(matches!(env.lock_obj(&object), Err(Error::JavaException)));
		drop(guard);
		assert!(env.exception_check());
		let pending: JThrowable<'_> = env.exception_occurred().expect("original exception is still pending");
		env.exception_clear();
		assert!(env.is_same_object(&exception, &pending)?);
		assert!(!holds_lock(env, &object)?);
		Ok(())
	})
	.expect("monitor release preserves the original exception without acquiring another lock");
}

#[test]
fn monitor_rejects_null_without_throwing() {
	util::attach_current_thread(|env| {
		let object: JObject<'static> = JObject::null();
		assert!(matches!(env.lock_obj(&object), Err(Error::NullPtr(_))));
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("null monitor is rejected before calling JNI");
}
