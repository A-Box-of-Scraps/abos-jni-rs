#![allow(missing_docs, reason = "Benchmarks are not library API.")]

use std::{hint::black_box, path::PathBuf, sync::LazyLock};

use abos_jni::{
	Env, InitArgsBuilder, JNIVersion, JValue, JavaVM,
	errors::Result,
	jni_str,
	objects::{JClass, JClassLoader, JObject, JString},
	signature::RuntimeMethodSignature,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

static VM: LazyLock<JavaVM> = LazyLock::new(|| {
	let args: abos_jni::InitArgs<'_> = InitArgsBuilder::new()
		.version(JNIVersion::V1_8)
		.build()
		.expect("benchmark JVM arguments");
	JavaVM::new(args).expect("benchmark JVM starts")
});

#[path = "../tests/util/fixtures.rs"]
mod fixtures;

fn dynamic_args(c: &mut Criterion) {
	let temporary: tempfile::TempDir = tempfile::tempdir().expect("fixture directory");
	let sources: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java");
	let classes: PathBuf = fixtures::compile_fixtures(&sources, temporary.path()).expect("compile Java fixtures");
	let bytes: Vec<u8> = std::fs::read(classes.join("com/example/DynamicCallArguments.class")).expect("fixture class");
	VM.attach_current_thread_for_scope(|env| -> Result<()> {
		let loader: JClassLoader<'_> = JClassLoader::get_system_class_loader(env)?;
		let class: JClass<'_> =
			env.define_class(Some(jni_str!("com/example/DynamicCallArguments")), &loader, &bytes)?;
		primitive_calls(c, env, &class)?;
		reference_calls(c, env, &class)
	})
	.expect("dynamic argument benchmarks succeed");
}

fn primitive_calls(c: &mut Criterion, env: &mut Env<'_>, class: &JClass<'_>) -> Result<()> {
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
		c.benchmark_group("call_args/dynamic/primitive");
	for count in [0, 1, 2, 4, 8, 9, 16] {
		let signature: RuntimeMethodSignature =
			RuntimeMethodSignature::from_str(format!("({})I", "I".repeat(count))).expect("valid signature");
		let args: Vec<JValue<'_>> = (0..count)
			.map(|index| JValue::Int(i32::try_from(index + 1).expect("small argument index")))
			.collect();
		let invoke = |env: &mut Env<'_>| -> Result<i32> {
			env.call_static_method(
				class,
				jni_str!("sumStatic"),
				signature.method_signature(),
				black_box(&args),
			)?
			.i()
		};
		assert_eq!(invoke(env)?, i32::try_from(count * (count + 1) / 2).expect("small sum"));
		group.bench_function(BenchmarkId::from_parameter(count), |b| {
			b.iter(|| black_box(invoke(env).expect("primitive call succeeds")));
		});
	}
	Ok(())
}

fn reference_calls(c: &mut Criterion, env: &mut Env<'_>, class: &JClass<'_>) -> Result<()> {
	let string: JString<'_> = env.new_string("value")?;
	let null: JObject<'_> = JObject::null();
	let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
		c.benchmark_group("call_args/dynamic/references");
	for count in [2_usize, 8, 9, 16] {
		let parameters: String = (0..count)
			.map(|index| if index % 2 == 0 { "I" } else { "Ljava/lang/String;" })
			.collect();
		let signature: RuntimeMethodSignature =
			RuntimeMethodSignature::from_str(format!("({parameters})I")).expect("valid signature");
		for (kind, value) in [("null", &null), ("non_null", string.as_ref())] {
			let args: Vec<JValue<'_>> = mixed_args(count, value);
			// Keep the existing reference collection and resolved-class assignability checks
			// inside timing. Packing improvements must not be confused with skipping validation.
			let invoke = |env: &mut Env<'_>| -> Result<i32> {
				env.call_static_method(class, jni_str!("mixed"), signature.method_signature(), black_box(&args))?
					.i()
			};
			let expected: usize = count.div_ceil(2) + if value.is_null() { 0 } else { 5 * (count / 2) };
			assert_eq!(invoke(env)?, i32::try_from(expected).expect("small sum"));
			group.bench_function(BenchmarkId::new(kind, count), |b| {
				b.iter(|| black_box(invoke(env).expect("reference call succeeds")));
			});
		}
	}
	Ok(())
}

fn mixed_args<'local>(count: usize, value: &'local JObject<'local>) -> Vec<JValue<'local>> {
	(0..count)
		.map(|index| {
			if index % 2 == 0 {
				JValue::Int(1)
			} else {
				JValue::Object(value)
			}
		})
		.collect()
}

criterion_group!(benches, dynamic_args);
criterion_main!(benches);
