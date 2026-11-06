#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use std::{
	sync::{Arc, Barrier, OnceLock},
	thread::spawn,
};

use abos_jni::{
	Env,
	errors::Error,
	jni_sig, jni_str,
	objects::{Auto, Global, IntoAuto as _, JObject, JString, JValue, Weak},
	sys::jint,
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn null_weak_local_upgrade_returns_none() {
	attach_current_thread(|env| {
		for weak in [Weak::<JObject<'static>>::null(), Weak::default()] {
			assert!(weak.is_garbage_collected(env)?);
			assert!(weak.upgrade_local(env)?.is_none());
		}
		for weak in [Weak::<JString<'static>>::null(), Weak::default()] {
			assert!(weak.upgrade_local(env)?.is_none());
		}
		Ok(())
	})
	.expect("null weak local upgrades succeed");
}

#[test]
fn null_weak_global_upgrade_returns_none() {
	attach_current_thread(|env| {
		for weak in [Weak::<JObject<'static>>::null(), Weak::default()] {
			assert!(weak.is_garbage_collected(env)?);
			assert!(weak.upgrade_global(env)?.is_none());
		}
		for weak in [Weak::<JString<'static>>::null(), Weak::default()] {
			assert!(weak.upgrade_global(env)?.is_none());
		}
		Ok(())
	})
	.expect("null weak global upgrades succeed");
}

#[test]
fn weak_upgrades_preserve_pending_exceptions() {
	attach_current_thread(|env| {
		let original: JString<'_> = env.new_string("live referent")?;
		let weak: Weak<JString<'static>> = env.new_weak_ref(&original)?;
		assert!(matches!(
			env.throw_new(jni_str!("java/lang/RuntimeException"), jni_str!("pending")),
			Err(Error::JavaException)
		));
		assert!(matches!(weak.upgrade_local(env), Err(Error::JavaException)));
		assert!(matches!(weak.upgrade_global(env), Err(Error::JavaException)));
		assert!(env.exception_check());
		env.exception_clear();
		Ok(())
	})
	.expect("weak upgrade errors preserve pending exceptions");
}

#[test]
fn weak_ref_works_in_other_threads() {
	const ITERS_PER_THREAD: usize = 10_000;

	attach_current_thread(|env| {
		static ATOMIC_INT: OnceLock<Weak<JObject<'static>>> = OnceLock::new();

		let atomic_integer_local: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/util/concurrent/atomic/AtomicInteger"),
				jni_sig!("(I)V"),
				&[JValue::from(0)],
			),
			env,
		)
		.auto();
		let atomic_integer_weak: Weak<JObject<'_>> = unwrap(env.new_weak_ref(&atomic_integer_local), env);

		ATOMIC_INT.set(atomic_integer_weak).expect("set succeeds");

		let mut join_handlers: Vec<std::thread::JoinHandle<()>> = Vec::new();

		// Test with a different number of threads (from 2 to 8)
		for thread_num in 2..9 {
			let barrier: Arc<Barrier> = Arc::new(Barrier::new(thread_num));

			for _ in 0..thread_num {
				let barrier: Arc<Barrier> = barrier.clone();
				let atomic_integer: &Weak<JObject<'_>> = ATOMIC_INT.get().expect("test bindings initialize");

				let jh: std::thread::JoinHandle<()> =
					spawn(move || increment_in_thread(&barrier, atomic_integer, ITERS_PER_THREAD));
				join_handlers.push(jh);
			}

			#[expect(
				clippy::iter_with_drain,
				reason = "Reuse the vector allocation in subsequent loop iterations."
			)]
			for jh in join_handlers.drain(..) {
				jh.join().expect("test worker completes without panicking");
			}

			let expected: i32 = jint::try_from(ITERS_PER_THREAD * thread_num).expect("test iteration count fits jint");
			assert_eq!(
				expected,
				unwrap(
					unwrap(
						env.call_method(
							&atomic_integer_local,
							jni_str!("getAndSet"),
							jni_sig!("(I)I"),
							&[JValue::from(0)]
						),
						env,
					)
					.i(),
					env,
				)
			);
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn typed_weak_reference_management() {
	attach_current_thread(|env| {
		let original: Auto<'_, JString<'_>> = env.new_string("weak reference")?.auto();
		let weak: Weak<JString<'static>> = env.new_weak_ref(&original)?;
		let local: JString<'_> = env.new_local_ref(&weak)?;
		let global: Global<JString<'static>> = env.new_global_ref(&weak)?;
		let cloned: Weak<JString<'static>> = env.new_weak_ref(&weak)?;

		drop(original);
		run_gc(env);

		assert_eq!(local.try_to_string(env)?, "weak reference");
		assert_eq!(global.try_to_string(env)?, "weak reference");
		assert!(env.is_same_object(&weak, &local)?);
		assert!(env.is_same_object(&global, &weak)?);
		assert!(env.is_same_object(&weak, &cloned)?);

		let upgraded: JString<'_> = cloned
			.upgrade_local(env)?
			.expect("strong references keep the string alive");
		let upgraded_global: Global<JString<'static>> = cloned
			.upgrade_global(env)?
			.expect("strong references keep the string alive");
		assert_eq!(upgraded.try_to_string(env)?, "weak reference");
		assert_eq!(upgraded_global.try_to_string(env)?, "weak reference");
		Ok(())
	})
	.expect("typed weak reference management succeeds");
}

#[test]
fn weak_ref_is_actually_weak() {
	attach_current_thread(|env| {
		// This test uses `with_local_frame` to work around issue #109.

		for _ in 0..100 {
			let obj_local: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
				env.with_local_frame_returning_local::<_, JObject, _>(2, |env| {
					env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])
				}),
				env,
			)
			.auto();

			let obj_weak: Weak<JObject<'_>> = unwrap(env.new_weak_ref(&obj_local), env);

			let obj_weak2: Weak<JObject<'_>> =
				unwrap(obj_weak.clone_in_jvm(env), env).expect("weak ref should not be null");

			run_gc(env);

			for obj_weak in &[&obj_weak, &obj_weak2] {
				assert_weak_live(env, obj_weak, &obj_local);
			}

			assert!(
				env.is_same_object(&obj_weak, &obj_weak2)
					.expect("is same object succeeds")
			);

			drop(obj_local);
			run_gc(env);

			for obj_weak in &[&obj_weak, &obj_weak2] {
				assert_weak_collected(env, obj_weak);
			}

			assert!(
				env.is_same_object(obj_weak, &obj_weak2)
					.expect("is same object succeeds")
			);
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn increment_in_thread(barrier: &Barrier, atomic_integer: &Weak<JObject<'static>>, iterations: usize) {
	attach_current_thread(|env| {
		barrier.wait();
		for _ in 0..iterations {
			let atomic_integer: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(atomic_integer.upgrade_local(env), env)
				.expect("AtomicInteger shouldn't have been GC'd yet")
				.auto();
			unwrap(
				unwrap(
					env.call_method(&atomic_integer, jni_str!("incrementAndGet"), jni_sig!("()I"), &[]),
					env,
				)
				.i(),
				env,
			);
		}
		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

fn run_gc(env: &mut Env) {
	unwrap(
		env.with_local_frame(1, |env| {
			env.call_static_method(jni_str!("java/lang/System"), jni_str!("gc"), jni_sig!("()V"), &[])?;
			Ok(())
		}),
		env,
	);
}

fn assert_weak_live(env: &mut Env, obj_weak: &Weak<JObject<'static>>, obj_local: &JObject<'_>) {
	{
		let obj_local_from_weak: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(obj_weak.upgrade_local(env), env)
			.expect("object shouldn't have been GC'd yet")
			.auto();

		assert!(!obj_local_from_weak.is_null());
		assert!(
			env.is_same_object(&obj_local_from_weak, obj_local)
				.expect("is same object succeeds")
		);
	}

	{
		let obj_global_from_weak: abos_jni::refs::Global<JObject<'_>> =
			unwrap(obj_weak.upgrade_global(env), env).expect("object shouldn't have been GC'd yet");

		assert!(!obj_global_from_weak.is_null());
		assert!(
			env.is_same_object(&obj_global_from_weak, obj_local)
				.expect("is same object succeeds")
		);
	}

	assert!(
		env.is_same_object(obj_weak, obj_local)
			.expect("is same object succeeds")
	);

	assert!(
		!obj_weak
			.is_garbage_collected(env)
			.expect("is garbage collected succeeds"),
		"`is_garbage_collected` returned incorrect value"
	);
}

fn assert_weak_collected(env: &mut Env, obj_weak: &Weak<JObject<'static>>) {
	assert!(matches!(env.new_local_ref(obj_weak), Err(Error::ObjectFreed)));
	assert!(matches!(env.new_global_ref(obj_weak), Err(Error::ObjectFreed)));

	{
		let obj_local_from_weak: Option<JObject<'_>> = unwrap(obj_weak.upgrade_local(env), env);

		assert!(obj_local_from_weak.is_none(), "object should have been GC'd");
	}

	{
		let obj_global_from_weak: Option<abos_jni::refs::Global<JObject<'_>>> =
			unwrap(obj_weak.upgrade_global(env), env);

		assert!(obj_global_from_weak.is_none(), "object should have been GC'd");
	}

	assert!(
		obj_weak
			.is_garbage_collected(env)
			.expect("is garbage collected succeeds"),
		"`is_garbage_collected` returned incorrect value"
	);
}
