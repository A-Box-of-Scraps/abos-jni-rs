#![allow(unsafe_code, reason = "Exercise the ownership contract of native Rust fields.")]
#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use std::{
	mem::ManuallyDrop,
	panic::{AssertUnwindSafe, catch_unwind},
	sync::{
		Arc, Mutex, MutexGuard, OnceLock,
		atomic::{AtomicUsize, Ordering},
	},
};

use abos_jni::{
	Env,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::JObject,
	sys::jlong,
};

struct Tracked {
	drops: Arc<AtomicUsize>,
	value: usize,
}

impl Drop for Tracked {
	fn drop(&mut self) {
		self.drops.fetch_add(1, Ordering::SeqCst);
	}
}

fn with_rust_field(test: impl FnOnce(&mut Env, &JObject, &Arc<AtomicUsize>) -> Result<()>) {
	static CLASS: OnceLock<()> = OnceLock::new();
	CLASS.get_or_init(|| {
		let out_dir: std::path::PathBuf = util::java_fixtures();
		util::attach_current_thread(|env| util::load_test_class(env, &out_dir, "TestRustFields"))
			.expect("load Rust field test class");
	});
	util::attach_current_thread(|env| {
		let obj: JObject<'_> = env.new_object(jni_str!("com/example/TestRustFields"), jni_sig!("()V"), &[])?;
		let drops: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
		let value = Tracked {
			drops: Arc::clone(&drops),
			value: 42,
		};
		// SAFETY: handle is an initially zero long field, accessed only through the Rust field API.
		unsafe { env.set_rust_field(&obj, jni_str!("handle"), value)? };
		test(env, &obj, &drops)
	})
	.expect("Rust field test succeeds");
}

fn handle(env: &mut Env, obj: &JObject) -> Result<jlong> {
	env.get_field(obj, jni_str!("handle"), jni_sig!("J"))?.j()
}

fn assert_take_succeeds(env: &mut Env, obj: &JObject, drops: &AtomicUsize, expected: usize) -> Result<()> {
	// SAFETY: handle owns the Tracked value installed by with_rust_field; no mutex guard remains.
	let value: Tracked = unsafe { env.take_rust_field(obj, jni_str!("handle"))? };
	assert_eq!(value.value, expected);
	assert_eq!(handle(env, obj)?, 0);
	assert_eq!(drops.load(Ordering::SeqCst), 0);
	drop(value);
	assert_eq!(drops.load(Ordering::SeqCst), 1);
	// SAFETY: The same long field is now empty, which must be reported without dereferencing it.
	let empty: Result<Tracked> = unsafe { env.take_rust_field(obj, jni_str!("handle")) };
	assert!(matches!(empty, Err(Error::NullPtr(_))));
	assert_eq!(drops.load(Ordering::SeqCst), 1);
	Ok(())
}

#[test]
fn take_rust_field_transfers_ownership() {
	with_rust_field(|env, obj, drops| assert_take_succeeds(env, obj, drops, 42));
}

#[test]
#[expect(
	clippy::significant_drop_tightening,
	reason = "The lock must span the failed take; ManuallyDrop::into_inner explicitly releases it after the assertions."
)]
fn take_locked_rust_field_preserves_ownership() {
	with_rust_field(|env, obj, drops| {
		let original: jlong = handle(env, obj)?;
		assert_ne!(original, 0);
		// SAFETY: handle owns the Tracked value installed by with_rust_field.
		let guard: MutexGuard<'_, Tracked> = unsafe { env.get_rust_field(obj, jni_str!("handle"))? };
		// A regression must fail the assertion without unlocking already-freed mutex storage during unwinding.
		let mut guard: ManuallyDrop<MutexGuard<'_, Tracked>> = ManuallyDrop::new(guard);
		// SAFETY: The field still owns the same Tracked value; an outstanding lock must produce an error.
		let result: Result<Tracked> = unsafe { env.take_rust_field(obj, jni_str!("handle")) };
		assert_eq!(drops.load(Ordering::SeqCst), 0, "failed take must not drop the value");
		assert!(matches!(result, Err(Error::TryLock)));
		assert_eq!(handle(env, obj)?, original);
		guard.value = 99;
		drop(ManuallyDrop::into_inner(guard));
		assert_take_succeeds(env, obj, drops, 99)
	});
}

#[test]
fn take_poisoned_rust_field_preserves_ownership() {
	with_rust_field(|env, obj, drops| {
		let original: jlong = handle(env, obj)?;
		assert_ne!(original, 0);
		let panic: std::thread::Result<()> = catch_unwind(AssertUnwindSafe(|| {
			// SAFETY: handle owns the Tracked value installed by with_rust_field.
			let _guard: MutexGuard<'_, Tracked> =
				unsafe { env.get_rust_field(obj, jni_str!("handle")) }.expect("lock Rust field before poisoning it");
			panic!("poison the Rust field mutex");
		}));
		assert!(panic.is_err());
		// SAFETY: Poisoning has not changed ownership or the type stored in the long field.
		let result: Result<Tracked> = unsafe { env.take_rust_field(obj, jni_str!("handle")) };
		assert_eq!(drops.load(Ordering::SeqCst), 0, "failed take must not drop the value");
		assert!(matches!(result, Err(Error::TryLock)));
		assert_eq!(handle(env, obj)?, original);
		// SAFETY: The unchanged handle still owns this live mutex, and the unwound guard has released its lock.
		unsafe { (&*(original as *mut Mutex<Tracked>)).clear_poison() };
		assert_take_succeeds(env, obj, drops, 42)
	});
}

#[test]
fn rust_field_operations_keep_owned_globals_alive_until_unlock() {
	with_rust_field(|env, obj, drops| {
		let global: abos_jni::refs::Global<JObject<'static>> = env.new_global_ref(obj)?;
		// SAFETY: The global refers to the same object holding the Tracked value installed by with_rust_field.
		let guard: MutexGuard<'_, Tracked> = unsafe { env.get_rust_field(global, jni_str!("handle"))? };
		assert_eq!(guard.value, 42);
		drop(guard);

		let global: abos_jni::refs::Global<JObject<'static>> = env.new_global_ref(obj)?;
		// SAFETY: The same field owns the Tracked value, and its mutex guard has been dropped.
		let value: Tracked = unsafe { env.take_rust_field(global, jni_str!("handle"))? };
		assert_eq!(handle(env, obj)?, 0);

		let global: abos_jni::refs::Global<JObject<'static>> = env.new_global_ref(obj)?;
		// SAFETY: Taking the value cleared this long field; transfer the same value back to Java.
		unsafe { env.set_rust_field(global, jni_str!("handle"), value)? };
		assert_take_succeeds(env, obj, drops, 42)
	});
}
