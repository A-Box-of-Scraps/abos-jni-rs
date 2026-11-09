use std::hint::black_box;

use abos_jni::jni_str;
use abos_jvmti::{Allocation, Env, JvmtiString, JvmtiStringList, RawMonitor, RawMonitorGuard, sys};
use criterion::{BenchmarkGroup, BenchmarkId, Criterion, Throughput, measurement::WallTime};

use super::with_jvmti;

pub fn resources(c: &mut Criterion) {
	with_jvmti(c, |c, env| {
		allocations(c, env);
		properties(c, env);
		monitors(c, env);
	});
}

fn allocations(c: &mut Criterion, env: &Env<'_>) {
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/allocation");
	for size in [64, 4096, 65536] {
		group.throughput(Throughput::Bytes(size as u64));
		group.bench_function(BenchmarkId::new("zeroed_drop", size), |b| {
			b.iter(|| {
				// SAFETY: The thread stays attached and the environment live through allocation and drop.
				let allocation: Allocation<'_, '_> =
					unsafe { env.allocate(black_box(size)) }.expect("allocation succeeds");
				black_box(allocation);
			});
		});
		group.bench_function(BenchmarkId::new("zeroed_explicit_free", size), |b| {
			b.iter(|| {
				// SAFETY: The thread stays attached and the environment live through allocation and free.
				let allocation: Allocation<'_, '_> =
					unsafe { env.allocate(black_box(size)) }.expect("allocation succeeds");
				black_box(allocation).free().expect("deallocation succeeds");
			});
		});
	}
}

fn properties(c: &mut Criterion, env: &Env<'_>) {
	c.bench_function("jvmti/system_property_drop", |b| {
		b.iter(|| {
			// SAFETY: Live phase, attached thread, outside callbacks; ownership ends before disposal.
			let value: JvmtiString<'_, '_> =
				unsafe { env.system_property(black_box(jni_str!("java.vm.name"))) }.expect("VM name is available");
			black_box(value);
		});
	});
	c.bench_function("jvmti/system_properties_drop", |b| {
		b.iter(|| {
			// SAFETY: Live phase, attached thread, outside callbacks; all nested allocations are dropped.
			let properties: JvmtiStringList<'_, '_> =
				unsafe { env.system_properties() }.expect("properties are available");
			black_box(properties);
		});
	});
}

fn monitors(c: &mut Criterion, env: &Env<'_>) {
	// SAFETY: Live phase on an attached thread. The monitor has no concurrent users and is destroyed here.
	let monitor: RawMonitor<'_, '_> =
		unsafe { env.create_raw_monitor(jni_str!("benchmark")) }.expect("monitor is created");
	// SAFETY: Env guarantees a live JVMTI >= 1.2 table; only the baseline layout is borrowed.
	let table: &sys::jvmtiInterface_1__1_2 = unsafe { &*(*env.as_raw()).cast() };
	let enter: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jrawMonitorID) -> sys::jvmtiError =
		table.RawMonitorEnter.expect("RawMonitorEnter is available");
	let exit: unsafe extern "system" fn(*mut sys::jvmtiEnv, sys::jrawMonitorID) -> sys::jvmtiError =
		table.RawMonitorExit.expect("RawMonitorExit is available");
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/monitor");
	group.bench_function("raw_cached_enter_exit", |b| {
		b.iter(|| {
			// SAFETY: This attached thread exclusively uses the live monitor, balancing every entry with an exit.
			unsafe {
				assert_eq!(
					enter(black_box(env.as_raw()), black_box(monitor.as_raw())),
					sys::JVMTI_ERROR_NONE
				);
				assert_eq!(
					exit(black_box(env.as_raw()), black_box(monitor.as_raw())),
					sys::JVMTI_ERROR_NONE
				);
			}
		});
	});
	group.bench_function("guard_drop", |b| {
		b.iter(|| {
			// SAFETY: The attached thread keeps the monitor/environment live until the guard exits.
			let guard: RawMonitorGuard<'_, '_, '_> =
				unsafe { black_box(&monitor).enter() }.expect("monitor entry succeeds");
			black_box(guard);
		});
	});
	group.bench_function("guard_unlock", |b| {
		b.iter(|| {
			// SAFETY: The attached thread keeps the monitor/environment live until the guard exits.
			let guard: RawMonitorGuard<'_, '_, '_> =
				unsafe { black_box(&monitor).enter() }.expect("monitor entry succeeds");
			black_box(guard).unlock().expect("monitor exit succeeds");
		});
	});
	drop(group);
	// SAFETY: All entries have been released; there are no other users or waiters.
	unsafe { monitor.destroy() }.expect("monitor is destroyed");
}
