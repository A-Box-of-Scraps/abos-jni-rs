use std::hint::black_box;

use abos_jni::{
	Env,
	errors::Result,
	jni_sig, jni_str,
	objects::{JIterator, JList, JObject},
	refs::{Auto, IntoAuto as _},
};
use criterion::{BenchmarkId, Criterion, Throughput};

use super::VM;

pub fn iterators(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> Result<()> {
		let object: JObject<'_> = env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[])?;
		let list: JList<'_> = JList::cast_local(env, object)?;
		let empty: Auto<'_, JIterator<'_>> = list.iter(env)?.auto();
		// Initialize the binding caches before measuring exhaustion without exception allocation.
		assert!(empty.next(env)?.is_none());
		c.bench_function("iterator/next_exhausted", |b| {
			b.iter(|| black_box(empty.next(env).expect("exhaustion check succeeds")));
		});
		drop(empty);

		let element: JObject<'_> = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		list.add(env, &element)?;
		let iter: Auto<'_, JIterator<'_>> = list.iter(env)?.auto();
		assert!(iter.has_next(env)?);
		c.bench_function("iterator/has_next_nonempty", |b| {
			// Isolate the additional Java dispatch now performed before every successful next.
			b.iter(|| black_box(iter.has_next(env).expect("hasNext succeeds")));
		});
		drop(iter);

		let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
			c.benchmark_group("iterator/traverse");
		for count in [1, 256] {
			list.clear(env)?;
			for _ in 0..count {
				list.add(env, &element)?;
			}
			// Warm up both nonempty and exhausted paths outside timing.
			traverse(env, &list)?;
			group.throughput(Throughput::Elements(count));
			group.bench_function(BenchmarkId::from_parameter(count), |b| {
				b.iter(|| traverse(env, &list).expect("iteration succeeds"));
			});
		}
		drop(group);
		env.delete_local_ref(element);
		env.delete_local_ref(list);
		Ok(())
	})
	.expect("iterator benchmarks succeed");
}

fn traverse(env: &mut Env<'_>, list: &JList<'_>) -> Result<()> {
	// Include iterator creation, local-reference cleanup, and the final exhaustion check.
	// N elements now require N next calls and N + 1 hasNext calls, not N + 1 next calls.
	let iter: Auto<'_, JIterator<'_>> = list.iter(env)?.auto();
	while let Some(object) = iter.next(env)? {
		black_box(object.auto());
	}
	Ok(())
}
