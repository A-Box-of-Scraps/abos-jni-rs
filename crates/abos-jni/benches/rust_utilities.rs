#![allow(missing_docs, reason = "Benchmarks are not library API.")]

use std::hint::black_box;

use abos_jni::{signature::RuntimeMethodSignature, strings::JNIString};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[path = "rust_utilities/call_args.rs"]
mod call_args;

#[path = "support/strings.rs"]
mod string_inputs;
use string_inputs::{STRING_CASES, STRING_REPEATS};

fn modified_utf8(c: &mut Criterion) {
	for (kind, unit) in STRING_CASES {
		let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
			c.benchmark_group(format!("mutf8/{kind}"));
		for repeats in STRING_REPEATS {
			let input: String = unit.repeat(repeats);
			let encoded = JNIString::new(&input);
			assert_eq!(encoded.to_str(), input);
			group.throughput(Throughput::Bytes(input.len() as u64));
			group.bench_function(BenchmarkId::new("encode_owned", repeats), |b| {
				b.iter(|| JNIString::new(black_box(&input)));
			});
			group.bench_function(BenchmarkId::new("decode_cow", repeats), |b| {
				b.iter(|| black_box(&encoded).to_str());
			});
		}
	}
}

fn signatures(c: &mut Criterion) {
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("signatures");
	for (name, descriptor) in [
		("primitive", "(I)I"),
		("object", "(Ljava/lang/String;)Ljava/lang/Object;"),
		("arrays", "([[I[Ljava/lang/String;J)Ljava/lang/Object;"),
	] {
		let parsed: RuntimeMethodSignature = RuntimeMethodSignature::from_str(descriptor).expect("signature parses");
		assert_eq!(parsed.method_signature().sig().to_str(), descriptor);
		group.bench_function(BenchmarkId::new("parse", name), |b| {
			b.iter(|| RuntimeMethodSignature::from_str(black_box(descriptor)).expect("signature parses"));
		});
		group.bench_function(BenchmarkId::new("borrow_cached", name), |b| {
			b.iter(|| black_box(&parsed).method_signature());
		});
	}
}

criterion_group!(benches, modified_utf8, signatures, call_args::call_args);
criterion_main!(benches);
