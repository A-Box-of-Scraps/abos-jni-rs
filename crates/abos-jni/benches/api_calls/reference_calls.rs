use std::hint::black_box;

use abos_jni::{
	JValue, bind_java_type,
	errors::Result,
	ids::JStaticMethodID,
	jni_sig, jni_str,
	objects::{JClass, JString},
	signature::{MethodSignature, Primitive, ReturnType},
};
use criterion::Criterion;

use super::VM;

bind_java_type! {
	JObjects => "java.util.Objects",
	methods { static fn equals(first: JObject, second: JObject) -> bool, }
}

pub fn reference_calls(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> Result<()> {
		let string: JString<'_> = env.new_string("reference argument")?;
		let class: JClass<'_> = env.find_class(jni_str!("java/util/Objects"))?;
		let signature: MethodSignature<'_, '_> = jni_sig!("(Ljava/lang/Object;Ljava/lang/Object;)Z");
		let method: JStaticMethodID = env.get_static_method_id(&class, jni_str!("equals"), &signature)?;
		// Initialize both binding and parameter-class caches before steady-state timing.
		assert!(JObjects::equals(env, &string, &string)?);
		let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> =
			c.benchmark_group("calls/objects_equals");
		group.bench_function("name_lookup", |b| {
			b.iter(|| {
				env.call_static_method(
					&class,
					jni_str!("equals"),
					&signature,
					&[JValue::Object(black_box(&string)), JValue::Object(black_box(&string))],
				)
				.expect("Objects.equals succeeds")
				.z()
				.expect("boolean result")
			});
		});
		group.bench_function("cached_id", |b| {
			b.iter(|| {
				// SAFETY: The cached ID is Objects.equals(Object,Object)Z; both arguments are live Strings.
				unsafe {
					env.call_static_method_unchecked(
						&class,
						method,
						ReturnType::Primitive(Primitive::Boolean),
						&[
							JValue::Object(black_box(&string)).as_jni(),
							JValue::Object(black_box(&string)).as_jni(),
						],
					)
				}
				.expect("Objects.equals succeeds")
				.z()
				.expect("boolean result")
			});
		});
		group.bench_function("generated_binding", |b| {
			b.iter(|| JObjects::equals(env, black_box(&string), black_box(&string)).expect("Objects.equals succeeds"));
		});
		drop(group);
		env.delete_local_ref(class);
		env.delete_local_ref(string);
		Ok(())
	})
	.expect("reference-argument call benchmarks succeed");
}
