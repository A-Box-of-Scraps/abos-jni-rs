//! Bounded lifecycle batches on `HotSpot` with a fixed 32 MiB Serial-GC heap.
//!
//! `HotSpot` retains disposed environments until GC cleanup; an unbounded churn
//! loop measures a growing native list, not steady-state wrapper overhead.
//! Each Criterion iteration here is one fixed-size batch, followed by a forced
//! GC before the next batch. Time is reported per batch, throughput per lifecycle.
//! `gc_excluded` measures only create/query-version/dispose calls and the loop;
//! `gc_included` also measures the following GC. Neither includes JVM startup or
//! attachment. These controlled costs are not representative of a real agent's
//! collector/heap, and even within a batch the environment list can grow.
//!
//! Run separately with `mise run bench:abos-jvmti:lifecycle`. This process uses
//! `HotSpot`-specific options to ensure explicit GC is enabled and synchronous.
#![allow(
	unsafe_code,
	reason = "Lifecycle benchmarks invoke JVMTI on an attached thread in the live phase."
)]
#![allow(missing_docs, reason = "Benchmarks are not library API.")]
#![cfg(feature = "invocation")]

use std::hint::black_box;

use abos_jni::{InitArgsBuilder, JNIVersion, JavaVM};
use abos_jvmti::{Env, sys};
use criterion::{
	BenchmarkGroup, BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
	measurement::WallTime,
};

#[path = "support/lifecycle.rs"]
mod lifecycle;

fn environment_lifecycle(c: &mut Criterion) {
	let args: abos_jni::InitArgs<'_> = InitArgsBuilder::new()
		.version(JNIVersion::V1_8)
		.option("-Xms32m")
		.option("-Xmx32m")
		.option("-XX:+UseSerialGC")
		.option("-XX:-DisableExplicitGC")
		.build()
		.expect("benchmark JVM arguments are valid");
	let vm: JavaVM = JavaVM::new(args).expect("lifecycle benchmark HotSpot JVM starts");
	vm.attach_current_thread_for_scope(|_| -> abos_jni::errors::Result<()> {
		bench_attached(c, &vm);
		Ok(())
	})
	.expect("scoped JNI attachment succeeds");
}

fn bench_attached(c: &mut Criterion, vm: &JavaVM) {
	// SAFETY: This thread is attached in the live phase; the VM outlives the environment.
	let control: Env<'_> = unsafe { Env::from_vm(vm) }.expect("control environment is created");
	// SAFETY: The attached thread is outside callbacks and the environment is live.
	let phase: sys::jvmtiPhase = unsafe { control.phase() }.expect("phase query succeeds");
	assert_eq!(phase, sys::JVMTI_PHASE_LIVE);
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/environment_lifecycle_batch");
	group.sampling_mode(SamplingMode::Flat);
	for size in [1_u64, 32, 256, 1024] {
		group.throughput(Throughput::Elements(size));
		for (name, include_gc) in [("gc_excluded", false), ("gc_included", true)] {
			let measure = |iterations| {
				lifecycle::measure_batches(
					iterations,
					size,
					include_gc,
					|| create_dispose(vm),
					|| collect(&control),
				)
			};
			group.bench_function(BenchmarkId::new(name, size), |b| b.iter_custom(measure));
		}
	}
	drop(group);
	// SAFETY: Batches have released all environments; the control has no resources or callbacks.
	unsafe { control.dispose() }.expect("control environment is disposed");
}

fn create_dispose(vm: &JavaVM) {
	// SAFETY: This attached thread can create environments, and the VM stays alive through disposal.
	let env: Env<'_> = unsafe { Env::from_vm(black_box(vm)) }.expect("environment is created");
	// SAFETY: This independent environment has no other handles, callbacks, or owned resources.
	unsafe { black_box(env).dispose() }.expect("environment is disposed");
}

fn collect(control: &Env<'_>) {
	// SAFETY: The control environment has a live JVMTI >= 1.2 table. Borrow only the baseline layout.
	let table: &sys::jvmtiInterface_1__1_2 = unsafe { &*(*control.as_raw()).cast() };
	let force_gc: unsafe extern "system" fn(*mut sys::jvmtiEnv) -> sys::jvmtiError = table
		.ForceGarbageCollection
		.expect("ForceGarbageCollection is available");
	// SAFETY: Live phase on an attached thread outside callbacks. No capability is required.
	let status: sys::jvmtiError = unsafe { force_gc(control.as_raw()) };
	assert_eq!(status, sys::JVMTI_ERROR_NONE, "GC reset succeeds");
}

criterion_group!(benches, environment_lifecycle);
criterion_main!(benches);
