#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use std::{
	sync::{Arc, Barrier, OnceLock},
	thread::spawn,
};

use abos_jni::{
	Env, jni_sig, jni_str,
	objects::{IntoAuto as _, JObject, JValue, Weak},
	sys::jint,
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn weak_ref_works_in_other_threads() {
	const ITERS_PER_THREAD: usize = 10_000;

	attach_current_thread(|env| {
		static ATOMIC_INT: OnceLock<Weak<JObject<'static>>> = OnceLock::new();

		let atomic_integer_local = unwrap(
			env.new_object(
				jni_str!("java/util/concurrent/atomic/AtomicInteger"),
				jni_sig!("(I)V"),
				&[JValue::from(0)],
			),
			env,
		)
		.auto();
		let atomic_integer_weak = unwrap(env.new_weak_ref(&atomic_integer_local), env);

		ATOMIC_INT.set(atomic_integer_weak).expect("set succeeds");

		let mut join_handlers = Vec::new();

		// Test with a different number of threads (from 2 to 8)
		for thread_num in 2..9 {
			let barrier = Arc::new(Barrier::new(thread_num));

			for _ in 0..thread_num {
				let barrier = barrier.clone();
				let atomic_integer = ATOMIC_INT.get().expect("test bindings initialize");

				let jh = spawn(move || increment_in_thread(&barrier, atomic_integer, ITERS_PER_THREAD));
				join_handlers.push(jh);
			}

			#[expect(
				clippy::iter_with_drain,
				reason = "Reuse the vector allocation in subsequent loop iterations."
			)]
			for jh in join_handlers.drain(..) {
				jh.join().expect("test worker completes without panicking");
			}

			let expected = jint::try_from(ITERS_PER_THREAD * thread_num).expect("test iteration count fits jint");
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
fn weak_ref_is_actually_weak() {
	attach_current_thread(|env| {
		// This test uses `with_local_frame` to work around issue #109.

		for _ in 0..100 {
			let obj_local = unwrap(
				env.with_local_frame_returning_local::<_, JObject, _>(2, |env| {
					env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])
				}),
				env,
			)
			.auto();

			let obj_weak = unwrap(env.new_weak_ref(&obj_local), env);

			let obj_weak2 = unwrap(obj_weak.clone_in_jvm(env), env).expect("weak ref should not be null");

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
			let atomic_integer = unwrap(atomic_integer.upgrade_local(env), env)
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
		let obj_local_from_weak = unwrap(obj_weak.upgrade_local(env), env)
			.expect("object shouldn't have been GC'd yet")
			.auto();

		assert!(!obj_local_from_weak.is_null());
		assert!(
			env.is_same_object(&obj_local_from_weak, obj_local)
				.expect("is same object succeeds")
		);
	}

	{
		let obj_global_from_weak =
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
	{
		let obj_local_from_weak = unwrap(obj_weak.upgrade_local(env), env);

		assert!(obj_local_from_weak.is_none(), "object should have been GC'd");
	}

	{
		let obj_global_from_weak = unwrap(obj_weak.upgrade_global(env), env);

		assert!(obj_global_from_weak.is_none(), "object should have been GC'd");
	}

	assert!(
		obj_weak
			.is_garbage_collected(env)
			.expect("is garbage collected succeeds"),
		"`is_garbage_collected` returned incorrect value"
	);
}
