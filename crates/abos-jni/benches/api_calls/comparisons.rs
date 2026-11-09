use std::{
	hint::black_box,
	time::{Duration, Instant},
};

use abos_jni::{
	Env,
	errors::Result,
	ids::JStaticMethodID,
	objects::{JClass, JObject},
	refs::IntoAuto as _,
};
use abos_jni_macros::bind_java_type;
use criterion::{BenchmarkGroup, BenchmarkId, Criterion, measurement::WallTime};

use super::{
	CLASS_MATH, CLASS_OBJECT, METHOD_MATH_ABS, SIG_MATH_ABS, SIG_OBJECT_CTOR, VM,
	calls::{jni_abs_safe, jni_int_call_static_unchecked},
};

bind_java_type! {
	JMath => "java.lang.Math",
	methods {
		static fn abs(value: jint) -> jint,
	}
}

pub fn calls(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> Result<()> {
		let class: JClass<'_> = env.find_class(CLASS_MATH)?;
		let method: JStaticMethodID = env.get_static_method_id(&class, METHOD_MATH_ABS, &SIG_MATH_ABS)?;
		assert_eq!(jni_abs_safe(env, -3), 3);
		assert_eq!(jni_int_call_static_unchecked(env, &class, method, -3), 3);
		// Initialize generated class/method caches before timing steady-state calls.
		assert_eq!(JMath::abs(env, -3)?, 3);
		let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
			c.benchmark_group("calls/math_abs");
		group.bench_function("name_lookup", |b| b.iter(|| jni_abs_safe(env, black_box(-3))));
		group.bench_function("cached_id", |b| {
			b.iter(|| jni_int_call_static_unchecked(env, &class, method, black_box(-3)));
		});
		group.bench_function("generated_binding", |b| {
			b.iter(|| JMath::abs(env, black_box(-3)).expect("Math.abs succeeds"));
		});
		drop(group);
		env.delete_local_ref(class);
		Ok(())
	})
	.expect("call benchmarks succeed");
}

pub fn references(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| references_attached(c, env))
		.expect("reference benchmarks succeed");
}

fn references_attached(c: &mut Criterion, env: &mut Env<'_>) -> Result<()> {
	let object: JObject<'_> = env.new_object(CLASS_OBJECT, &SIG_OBJECT_CTOR, &[])?;
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("references");
	for count in [1, 32, 256] {
		reference_batch(&mut group, env, &object, count);
	}
	group.bench_function("global_create_drop", |b| {
		b.iter(|| env.new_global_ref(&object).expect("global reference succeeds"));
	});
	drop(group);
	env.delete_local_ref(object);
	Ok(())
}

fn reference_batch(group: &mut BenchmarkGroup<'_, WallTime>, env: &mut Env<'_>, object: &JObject<'_>, count: usize) {
	group.bench_function(BenchmarkId::new("local_explicit_delete", count), |b| {
		b.iter(|| {
			for _ in 0..count {
				let local: JObject<'_> = env.new_local_ref(object).expect("local reference succeeds");
				env.delete_local_ref(local);
			}
		});
	});
	group.bench_function(BenchmarkId::new("local_auto", count), |b| {
		b.iter(|| {
			for _ in 0..count {
				black_box(env.new_local_ref(object).expect("local reference succeeds").auto());
			}
		});
	});
	let batch = || {
		env.with_local_frame(count, |env| -> Result<()> {
			for _ in 0..count {
				black_box(env.new_local_ref(object)?);
			}
			Ok(())
		})
		.expect("local frame succeeds");
	};
	group.bench_function(BenchmarkId::new("local_frame_batch", count), |b| b.iter(batch));
}

fn attachment_batch(iterations: u64, already_attached: bool) -> Duration {
	// A fresh worker guarantees a detached initial state. Thread creation/join and
	// the permanent attachment are excluded from the duration returned to Criterion.
	std::thread::spawn(move || {
		if already_attached {
			VM.attach_current_thread(|_| -> Result<()> { Ok(()) })
				.expect("permanent attachment succeeds");
		}
		let start: Instant = Instant::now();
		for _ in 0..iterations {
			VM.attach_current_thread_for_scope(|_| -> Result<()> { Ok(()) })
				.expect("scoped attachment succeeds");
		}
		let elapsed: Duration = start.elapsed();
		if already_attached {
			VM.detach_current_thread().expect("worker detaches");
		}
		elapsed
	})
	.join()
	.expect("attachment worker succeeds")
}

pub fn attachment(c: &mut Criterion) {
	std::sync::LazyLock::force(&VM);
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
		c.benchmark_group("attachment/scoped_noop");
	group.bench_function("attach_detach", |b| {
		b.iter_custom(|iterations| attachment_batch(iterations, false));
	});
	group.bench_function("already_attached", |b| {
		b.iter_custom(|iterations| attachment_batch(iterations, true));
	});
}
