use std::hint::black_box;

use abos_jni::{Env, errors::Result, objects::JString, strings::JNIString};
use criterion::{BenchmarkId, Criterion, Throughput};

use super::{
	VM,
	string_inputs::{STRING_CASES, STRING_REPEATS},
};

pub fn strings(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> Result<()> {
		for (kind, unit) in STRING_CASES {
			string_kind(c, env, kind, unit)?;
		}
		Ok(())
	})
	.expect("string benchmarks succeed");
}

fn string_kind(c: &mut Criterion, env: &mut Env<'_>, kind: &str, unit: &str) -> Result<()> {
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
		c.benchmark_group(format!("strings/{kind}"));
	for repeats in STRING_REPEATS {
		let input: String = unit.repeat(repeats);
		let encoded = JNIString::new(&input);
		let java: JString<'_> = JString::from_jni_str(env, &encoded)?;
		assert_eq!(java.try_to_string(env)?, input);
		let from_utf8: JString<'_> = JString::from_str(env, &input)?;
		assert_eq!(from_utf8.try_to_string(env)?, input);
		env.delete_local_ref(from_utf8);
		group.throughput(Throughput::Bytes(input.len() as u64));
		group.bench_function(BenchmarkId::new("from_utf8", repeats), |b| {
			b.iter(|| {
				let value: JString<'_> = JString::from_str(env, black_box(&input)).expect("string creation succeeds");
				env.delete_local_ref(value);
			});
		});
		group.bench_function(BenchmarkId::new("from_preencoded_mutf8", repeats), |b| {
			b.iter(|| {
				let value: JString<'_> =
					JString::from_jni_str(env, black_box(&encoded)).expect("string creation succeeds");
				env.delete_local_ref(value);
			});
		});
		group.bench_function(BenchmarkId::new("to_utf8", repeats), |b| {
			b.iter(|| java.try_to_string(env).expect("string conversion succeeds"));
		});
		env.delete_local_ref(java);
	}
	drop(group);
	Ok(())
}
