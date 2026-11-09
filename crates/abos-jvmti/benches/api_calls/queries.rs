use std::hint::black_box;

use abos_jvmti::{Env, sys};
use criterion::{BenchmarkGroup, Criterion, measurement::WallTime};

use super::{VM, with_jvmti};

pub fn queries(c: &mut Criterion) {
	with_jvmti(c, |c, env| {
		phase(c, env);
		construction(c, env);
		c.bench_function("jvmti/capabilities", |b| {
			b.iter(|| {
				// SAFETY: with_jvmti keeps this thread attached in the live phase, outside callbacks.
				unsafe { black_box(env).capabilities() }.expect("capabilities query succeeds")
			});
		});
		c.bench_function("jvmti/environment_local_storage", |b| {
			b.iter(|| {
				// SAFETY: The environment is live, thread attached, and no other user changes storage.
				unsafe { black_box(env).environment_local_storage() }.expect("storage query succeeds")
			});
		});
	});
}

fn phase(c: &mut Criterion, env: &Env<'_>) {
	// SAFETY: Env guarantees a live JVMTI >= 1.2 table. Borrow only the baseline layout.
	let table: &sys::jvmtiInterface_1__1_2 = unsafe { &*(*env.as_raw()).cast() };
	let get_phase: unsafe extern "system" fn(*mut sys::jvmtiEnv, *mut sys::jvmtiPhase) -> sys::jvmtiError =
		table.GetPhase.expect("GetPhase is available");
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/phase");
	group.bench_function("raw_cached_function", |b| {
		b.iter(|| {
			let mut value: sys::jvmtiPhase = 0;
			// SAFETY: The environment is live, the thread attached, and the output writable.
			let status: sys::jvmtiError = unsafe { get_phase(black_box(env.as_raw()), &raw mut value) };
			assert_eq!(status, sys::JVMTI_ERROR_NONE);
			black_box(value)
		});
	});
	group.bench_function("wrapper", |b| {
		b.iter(|| {
			// SAFETY: with_jvmti keeps this thread attached and outside callbacks.
			unsafe { black_box(env).phase() }.expect("phase query succeeds")
		});
	});
}

fn construction(c: &mut Criterion, env: &Env<'_>) {
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/environment");
	group.bench_function("cached_version", |b| b.iter(|| black_box(env).version()));
	group.bench_function("borrow_known_version", |b| {
		b.iter(|| {
			// SAFETY: The pointer and actual version belong to the same live environment.
			unsafe { Env::from_raw_with_version(black_box(env.as_raw()), black_box(env.version())) }
				.expect("environment is valid")
		});
	});
	group.bench_function("borrow_query_version", |b| {
		b.iter(|| {
			// SAFETY: The environment is live and this attached thread is outside callbacks.
			unsafe { Env::from_raw(black_box(env.as_raw())) }.expect("version query succeeds")
		});
	});
	group.bench_function("create_dispose", |b| {
		b.iter(|| {
			// SAFETY: VM remains live and this attached thread can create independent environments.
			let independent: Env<'_> = unsafe { Env::from_vm(black_box(&VM)) }.expect("environment is created");
			// SAFETY: No resources, callbacks, or other users reference this independent environment.
			unsafe { black_box(independent).dispose() }.expect("environment is disposed");
		});
	});
}
