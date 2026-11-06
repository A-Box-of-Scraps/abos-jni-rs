#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use std::{
	sync::{Arc, Barrier, OnceLock},
	thread::spawn,
};

use abos_jni::{
	jni_sig, jni_str,
	objects::{Global, IntoAuto as _, JObject, JValue},
	sys::jint,
};

mod util;
use util::{attach_current_thread, unwrap};

#[test]
fn global_ref_works_in_other_threads() {
	static ATOMIC_INT: OnceLock<Global<JObject<'static>>> = OnceLock::new();

	const ITERS_PER_THREAD: usize = 10_000;

	let atomic_integer: Global<JObject<'_>> = attach_current_thread(|env| {
		let local_ref: abos_jni::refs::Auto<'_, JObject<'_>> = unwrap(
			env.new_object(
				jni_str!("java/util/concurrent/atomic/AtomicInteger"),
				jni_sig!("(I)V"),
				&[JValue::from(0)],
			),
			env,
		)
		.auto();
		env.new_global_ref(&local_ref)
	})
	.expect("attached JNI callback succeeds");

	ATOMIC_INT.set(atomic_integer).expect("set succeeds");

	let mut join_handlers: Vec<std::thread::JoinHandle<()>> = Vec::new();

	// Test with a different number of threads (from 2 to 8)
	for thread_num in 2..9 {
		let barrier: Arc<Barrier> = Arc::new(Barrier::new(thread_num));

		for _ in 0..thread_num {
			let barrier: Arc<Barrier> = barrier.clone();
			let atomic_integer: &Global<JObject<'_>> = ATOMIC_INT.get().expect("test bindings initialize");

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

		attach_current_thread(|env| {
			let atomic_integer: &Global<JObject<'_>> = ATOMIC_INT.get().expect("test bindings initialize");
			let expected: i32 = jint::try_from(ITERS_PER_THREAD * thread_num).expect("test iteration count fits jint");
			assert_eq!(
				expected,
				unwrap(
					unwrap(
						env.call_method(
							atomic_integer,
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
			Ok(())
		})
		.expect("attached JNI callback succeeds");
	}
}

fn increment_in_thread(barrier: &Barrier, atomic_integer: &Global<JObject<'static>>, iterations: usize) {
	attach_current_thread(|env| {
		barrier.wait();
		for _ in 0..iterations {
			unwrap(
				unwrap(
					env.call_method(atomic_integer, jni_str!("incrementAndGet"), jni_sig!("()I"), &[]),
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
