use std::hint::black_box;

use abos_jni::{
	Env,
	elements::{AutoElements, ReleaseMode},
	errors::Result,
	objects::JIntArray,
};
use criterion::{BenchmarkId, Criterion, Throughput};

use super::VM;

pub fn arrays(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| arrays_attached(c, env))
		.expect("array benchmarks succeed");
}

fn arrays_attached(c: &mut Criterion, env: &mut Env<'_>) -> Result<()> {
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("arrays/i32");
	for size in [16, 1024, 65_536] {
		let input: Vec<i32> = (0..size)
			.map(|index| i32::try_from(index % 127).expect("value fits i32"))
			.collect();
		let array: JIntArray<'_> = env.new_int_array(size)?;
		let mut output: Vec<i32> = vec![0; size];
		{
			// SAFETY: This fresh array is thread-private, with no other elements guard.
			let mut elements: AutoElements<'_, i32, &JIntArray<'_>> =
				unsafe { array.get_elements(env, ReleaseMode::CopyBack)? };
			elements.copy_from_slice(&input);
		}
		array.get_region(env, 0, &mut output)?;
		assert_eq!(output, input);
		output.fill(0);
		array.set_region(env, 0, &output)?;
		array.set_region(env, 0, &input)?;
		array.get_region(env, 0, &mut output)?;
		assert_eq!(output, input);
		{
			// SAFETY: The array is private to this thread and only one elements guard exists.
			let elements: AutoElements<'_, i32, &JIntArray<'_>> =
				unsafe { array.get_elements(env, ReleaseMode::NoCopyBack)? };
			assert_eq!(&*elements, input);
		}
		group.throughput(Throughput::Bytes((size * size_of::<i32>()) as u64));
		group.bench_function(BenchmarkId::new("read_region", size), |b| {
			b.iter(|| {
				array.get_region(env, 0, &mut output).expect("array read succeeds");
				black_box(&output);
			});
		});
		group.bench_function(BenchmarkId::new("read_elements", size), |b| {
			b.iter(|| {
				// SAFETY: The private array is not shared, and this guard is released each iteration.
				let elements: AutoElements<'_, i32, &JIntArray<'_>> =
					unsafe { array.get_elements(env, ReleaseMode::NoCopyBack) }.expect("array access succeeds");
				output.copy_from_slice(&elements);
				black_box(&output);
			});
		});
		group.bench_function(BenchmarkId::new("write_region", size), |b| {
			b.iter(|| {
				array
					.set_region(env, 0, black_box(&input))
					.expect("array write succeeds");
			});
		});
		group.bench_function(BenchmarkId::new("write_elements", size), |b| {
			b.iter(|| {
				// SAFETY: The private array is not shared, and this guard is released each iteration.
				let mut elements: AutoElements<'_, i32, &JIntArray<'_>> =
					unsafe { array.get_elements(env, ReleaseMode::CopyBack) }.expect("array access succeeds");
				elements.copy_from_slice(black_box(&input));
			});
		});
		array.get_region(env, 0, &mut output)?;
		assert_eq!(output, input);
		env.delete_local_ref(array);
	}
	drop(group);
	Ok(())
}
