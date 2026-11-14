use std::hint::black_box;

use abos_jni::{JValue, objects::JObject, sys::jvalue};
use criterion::{BenchmarkId, Criterion};

// Benchmark the implementation itself without making an internal helper public API.
#[path = "../../src/api/env/call_args.rs"]
mod packing;

pub fn call_args(c: &mut Criterion) {
	let null: JObject<'_> = JObject::null();
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
		c.benchmark_group("call_args/packing");
	for count in [0, 1, 2, 4, 8, 9, 16, 32] {
		let args: Vec<JValue<'_>> = (0..count)
			.map(|index| match index % 3 {
				0 => JValue::Int(index),
				1 => JValue::Long(i64::from(index)),
				_ => JValue::Object(&null),
			})
			.collect();
		group.bench_function(BenchmarkId::new("vec", count), |b| {
			b.iter(|| {
				let converted: Vec<jvalue> = black_box(&args).iter().map(JValue::as_jni).collect();
				black_box(converted.as_slice());
			});
		});
		group.bench_function(BenchmarkId::new("stack_8", count), |b| {
			b.iter(|| packing::with_jni_args(black_box(&args), observe));
		});
	}
}

const fn observe(converted: &[jvalue]) {
	black_box(converted);
}
