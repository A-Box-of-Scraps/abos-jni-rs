#![allow(missing_docs, reason = "Benchmarks are not library API.")]

use std::hint::black_box;

use abos_jvmti::{JvmtiVersion, sys};
use criterion::{BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main, measurement::WallTime};

fn versions(c: &mut Criterion) {
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/version");
	for version in [JvmtiVersion::V1_2, JvmtiVersion::V21, JvmtiVersion::V25] {
		assert_eq!(JvmtiVersion::from_raw(version.as_raw()), Ok(version));
		group.bench_function(BenchmarkId::new("validate", version), |b| {
			b.iter(|| JvmtiVersion::from_raw(black_box(version.as_raw())).expect("version is valid"));
		});
		group.bench_function(BenchmarkId::new("components", version), |b| {
			b.iter(|| {
				let value: JvmtiVersion = black_box(version);
				(value.major(), value.minor(), value.micro())
			});
		});
	}
}

fn compatibility(c: &mut Criterion) {
	let mut group: BenchmarkGroup<'_, WallTime> = c.benchmark_group("jvmti/compatibility");
	let mut baseline: sys::jvmtiCapabilities = sys::jvmtiCapabilities::default();
	baseline.set_can_tag_objects(true);
	let mut modern: sys::jvmtiCapabilities = baseline;
	modern.set_can_support_virtual_threads(true);
	for (name, capabilities, version) in [
		("baseline", baseline, JvmtiVersion::V1_2),
		("virtual_threads", modern, JvmtiVersion::V21),
	] {
		assert_eq!(capabilities.required_version(), version.as_raw());
		group.bench_function(BenchmarkId::new("capability_version", name), |b| {
			b.iter(|| black_box(&capabilities).required_version());
		});
	}
	for (name, event) in [
		("method_entry", sys::JVMTI_EVENT_METHOD_ENTRY),
		("virtual_thread_start", sys::JVMTI_EVENT_VIRTUAL_THREAD_START),
	] {
		group.bench_function(BenchmarkId::new("event_version", name), |b| {
			b.iter(|| sys::jvmti_event_version(black_box(event)).expect("event is known"));
		});
	}
}

criterion_group!(benches, versions, compatibility);
criterion_main!(benches);
