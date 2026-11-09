#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::signature::MethodSignature;
use abos_jni::strings::JNIStr;
use abos_jni::{jni_sig, jni_str};

use abos_jni::objects::{Global, IntoAuto as _, JString};
use abos_jni::{
	InitArgsBuilder, JNIVersion, JavaVM,
	descriptors::Desc,
	objects::{JClass, JObject, JValue},
};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

#[path = "api_calls/arrays.rs"]
mod arrays;
#[path = "api_calls/calls.rs"]
mod calls;
#[path = "api_calls/comparisons.rs"]
mod comparisons;
#[path = "api_calls/reference_calls.rs"]
mod reference_calls;
#[path = "support/strings.rs"]
mod string_inputs;
#[path = "api_calls/strings.rs"]
mod strings;
use calls::{
	jni_abs_safe, jni_hash_safe, jni_int_call_static_unchecked, jni_int_call_unchecked, jni_local_date_time_of_safe,
	jni_object_call_static_unchecked, native_abs,
};

static CLASS_MATH: &JNIStr = jni_str!("java/lang/Math");
static CLASS_OBJECT: &JNIStr = jni_str!("java/lang/Object");
static CLASS_LOCAL_DATE_TIME: &JNIStr = jni_str!("java/time/LocalDateTime");
static METHOD_MATH_ABS: &JNIStr = jni_str!("abs");
static METHOD_OBJECT_HASH_CODE: &JNIStr = jni_str!("hashCode");
static METHOD_CTOR: &JNIStr = jni_str!("<init>");
static METHOD_LOCAL_DATE_TIME_OF: &JNIStr = jni_str!("of");
static SIG_OBJECT_CTOR: MethodSignature = jni_sig!("()V");
static SIG_MATH_ABS: MethodSignature = jni_sig!("(I)I");
static SIG_OBJECT_HASH_CODE: MethodSignature = jni_sig!("()I");
static SIG_LOCAL_DATE_TIME_OF: MethodSignature = jni_sig!("(IIIIIII)Ljava/time/LocalDateTime;");

// 32 characters
static TEST_STRING_UNICODE: &str = "_񍷕㳧~δ򗊁᪘׷ġ˥쩽|ņ/򖕡ٶԦ萴퀉֒ٞHy󢕒%ӓ娎񢞊ăꊦȮ񳗌";

static VM: LazyLock<JavaVM> = LazyLock::new(|| {
	let args: abos_jni::InitArgs<'_> = InitArgsBuilder::new()
		.version(JNIVersion::V1_8)
		.build()
		.expect("benchmark JVM arguments are valid");
	JavaVM::new(args).expect("benchmark JVM starts")
});

fn native_call_function(c: &mut Criterion) {
	c.bench_function("native_call_function", |b| {
		b.iter(|| {
			black_box(native_abs(black_box(-3)));
		});
	});
}

fn jni_call_static_abs_method_safe(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		c.bench_function("jni_call_static_abs_method_safe", |b| b.iter(|| jni_abs_safe(env, -3)));
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_static_abs_method_unchecked_str(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: &JNIStr = CLASS_MATH;
		let method_id: abos_jni::ids::JStaticMethodID = env
			.get_static_method_id(class, METHOD_MATH_ABS, &SIG_MATH_ABS)
			.expect("test static method can be resolved");

		c.bench_function("jni_call_static_abs_method_unchecked_str", |b| {
			b.iter(|| jni_int_call_static_unchecked(env, class, method_id, -3));
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_static_abs_method_unchecked_jclass(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: abos_jni::refs::Auto<'_, JClass<'_>> =
			Desc::<JClass>::lookup(CLASS_MATH, env).expect("lookup succeeds");
		let method_id: abos_jni::ids::JStaticMethodID = env
			.get_static_method_id(&class, METHOD_MATH_ABS, &SIG_MATH_ABS)
			.expect("test static method can be resolved");

		c.bench_function("jni_call_static_abs_method_unchecked_jclass", |b| {
			b.iter(|| jni_int_call_static_unchecked(env, &class, method_id, -3));
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_static_date_time_method_safe(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		c.bench_function("jni_call_static_date_time_method_safe", |b| {
			b.iter(|| {
				let obj: JObject<'_> = jni_local_date_time_of_safe(env, [1; 7]);
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_static_date_time_method_unchecked_jclass(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: abos_jni::refs::Auto<'_, JClass<'_>> =
			Desc::<JClass>::lookup(CLASS_LOCAL_DATE_TIME, env).expect("lookup succeeds");
		let method_id: abos_jni::ids::JStaticMethodID = env
			.get_static_method_id(&class, METHOD_LOCAL_DATE_TIME_OF, &SIG_LOCAL_DATE_TIME_OF)
			.expect("test static method can be resolved");

		c.bench_function("jni_call_static_date_time_method_unchecked_jclass", |b| {
			b.iter(|| {
				let obj: JObject<'_> = jni_object_call_static_unchecked(
					env,
					&class,
					method_id,
					&[
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
						JValue::Int(1).as_jni(),
					],
				);
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_object_hash_method_safe(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let s: JString<'_> = env.new_string("").expect("test string can be allocated");
		let obj: JObject<'_> = black_box(JObject::from(s));

		c.bench_function("jni_call_object_hash_method_safe", |b| {
			b.iter(|| jni_hash_safe(env, &obj));
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_call_object_hash_method_unchecked(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let s: JString<'_> = env.new_string("").expect("test string can be allocated");
		let obj: JObject<'_> = black_box(JObject::from(s));
		let obj_class: JClass<'_> = env.get_object_class(&obj).expect("get object class succeeds");
		let method_id: abos_jni::ids::JMethodID = env
			.get_method_id(&obj_class, METHOD_OBJECT_HASH_CODE, &SIG_OBJECT_HASH_CODE)
			.expect("test method can be resolved");

		c.bench_function("jni_call_object_hash_method_unchecked", |b| {
			b.iter(|| jni_int_call_unchecked(env, &obj, method_id));
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_new_object_str(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: &JNIStr = CLASS_OBJECT;

		c.bench_function("jni_new_object_str", |b| {
			b.iter(|| {
				let obj: JObject<'_> = env
					.new_object(class, &SIG_OBJECT_CTOR, &[])
					.expect("test object can be allocated");
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_new_object_by_id_str(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: &JNIStr = CLASS_OBJECT;
		let ctor_id: abos_jni::ids::JMethodID = env
			.get_method_id(class, METHOD_CTOR, &SIG_OBJECT_CTOR)
			.expect("test method can be resolved");

		c.bench_function("jni_new_object_by_id_str", |b| {
			b.iter(|| {
				// SAFETY: The constructor ID was resolved for this class with a no-argument signature.
				let obj: JObject<'_> =
					unsafe { env.new_object_unchecked(class, ctor_id, &[]) }.expect("new object unchecked succeeds");
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_new_object_jclass(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: abos_jni::refs::Auto<'_, JClass<'_>> =
			Desc::<JClass>::lookup(CLASS_OBJECT, env).expect("lookup succeeds");

		c.bench_function("jni_new_object_jclass", |b| {
			b.iter(|| {
				let obj: JObject<'_> = env
					.new_object(&class, &SIG_OBJECT_CTOR, &[])
					.expect("test object can be allocated");
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_new_object_by_id_jclass(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: abos_jni::refs::Auto<'_, JClass<'_>> =
			Desc::<JClass>::lookup(CLASS_OBJECT, env).expect("lookup succeeds");
		let ctor_id: abos_jni::ids::JMethodID = env
			.get_method_id(&class, METHOD_CTOR, &SIG_OBJECT_CTOR)
			.expect("test method can be resolved");

		c.bench_function("jni_new_object_by_id_jclass", |b| {
			b.iter(|| {
				// SAFETY: The constructor ID was resolved for this class with a no-argument signature.
				let obj: JObject<'_> =
					unsafe { env.new_object_unchecked(&class, ctor_id, &[]) }.expect("new object unchecked succeeds");
				env.delete_local_ref(obj);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_new_global_ref(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: &JNIStr = CLASS_OBJECT;
		let obj: JObject<'_> = env
			.new_object(class, &SIG_OBJECT_CTOR, &[])
			.expect("test object can be allocated");
		let global_ref: Global<JObject<'_>> = env.new_global_ref(&obj).expect("global reference can be allocated");
		env.delete_local_ref(obj);

		c.bench_function("jni_new_global_ref", |b| {
			b.iter(|| {
				env.new_global_ref(&global_ref)
					.expect("global reference can be allocated")
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// Checks the overhead of checking if exception has occurred.
///
/// Such checks are required each time a Java method is called, but
/// can be omitted if we call a JNI method that returns an error status.
///
/// See also #58
fn jni_check_exception(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		c.bench_function("jni_check_exception", |b| b.iter(|| env.exception_check()));
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_get_java_vm(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		c.bench_function("jni_get_java_vm", |b| {
			b.iter(|| env.get_java_vm().expect("Java VM can be retrieved"));
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn jni_get_string(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let string: JString<'_> = env
			.new_string(TEST_STRING_UNICODE)
			.expect("test string can be allocated");

		assert_eq!(
			string
				.try_to_string(env)
				.expect("test Java string can be converted to Rust"),
			TEST_STRING_UNICODE
		);

		c.bench_function("jni_get_string", |b| {
			b.iter(|| {
				let s: String = string
					.try_to_string(env)
					.expect("test Java string can be converted to Rust");
				black_box(s);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// A benchmark measuring Push/PopLocalFrame overhead.
///
/// Such operations are *required* if one attaches a long-running
/// native thread to the JVM because there is no 'return-from-native-method'
/// event when created local references are freed, hence no way for
/// the JVM to know that the local references are no longer used in the native code.
fn jni_noop_with_local_frame(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		const LOCAL_FRAME_SIZE: usize = 32;
		let mut operation = || {
			env.with_local_frame(LOCAL_FRAME_SIZE, |_| -> Result<_, abos_jni::errors::Error> { Ok(()) })
				.expect("local-frame callback succeeds");
		};
		c.bench_function("jni_noop_with_local_frame", |b| {
			b.iter(&mut operation);
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// Measures local-frame object creation and return, including deletion of the returned local reference.
fn jni_with_local_frame_returning_local(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		const LOCAL_FRAME_SIZE: usize = 32;

		let class: JClass<'_> = env.find_class(CLASS_OBJECT).expect("test class can be resolved");
		let mut operation = || {
			let local: JObject<'_> = env
				.with_local_frame_returning_local::<_, JObject, _>(LOCAL_FRAME_SIZE, |env| {
					env.new_object(&class, &SIG_OBJECT_CTOR, &[])
				})
				.expect("local-frame object creation succeeds");
			env.delete_local_ref(local);
		};
		c.bench_function("jni_with_local_frame_returning_local", |b| {
			b.iter(&mut operation);
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// Measures local-frame object creation and return via a temporary global reference,
/// including deletion of both the global and the resulting local reference.
fn jni_with_local_frame_returning_global_to_local(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		const LOCAL_FRAME_SIZE: usize = 32;

		let class: JClass<'_> = env.find_class(CLASS_OBJECT).expect("test class can be resolved");
		let mut operation = || {
			let global: Global<JObject<'_>> = env
				.with_local_frame::<_, Global<JObject<'static>>, abos_jni::errors::Error>(LOCAL_FRAME_SIZE, |env| {
					let local: JObject<'_> = env.new_object(&class, &SIG_OBJECT_CTOR, &[])?;
					let global: Global<JObject<'_>> = env.new_global_ref(local)?;
					Ok(global)
				})
				.expect("test object can be allocated");
			let local: JObject<'_> = env.new_local_ref(global).expect("local reference can be allocated");
			env.delete_local_ref(local);
		};
		c.bench_function("jni_with_local_frame_returning_global_to_local", |b| {
			b.iter(&mut operation);
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// A benchmark of the overhead of repeatedly allocating/freeing a string with JNI
///
/// This provides a baseline to compare with the cost of making repeat attachments or checking for
/// existing attachments.
fn jni_new_string_within_single_thread_attachment(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		c.bench_function("jni_new_string_within_single_thread_attachment", |b| {
			b.iter(|| {
				black_box(
					JString::from_jni_str(env, jni_str!("Test"))
						.expect("test JNI string can be allocated")
						.auto(),
				);
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

/// A benchmark of the overhead of repeatedly attaching and detaching a native thread, with a scope.
fn jni_new_string_with_repeat_scoped_thread_attachments(c: &mut Criterion) {
	c.bench_function("jni_new_string_with_repeat_scoped_thread_attachments", |b| {
		b.iter(|| {
			VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
				black_box(
					JString::from_jni_str(env, jni_str!("Test"))
						.expect("test JNI string can be allocated")
						.auto(),
				);
				Ok(())
			})
			.expect("scoped JNI callback succeeds");
		});
	});
}

/// A benchmark of the overhead of repeatedly attaching and detaching a native thread.
///
/// The request to attach the thread permanently should mean the cost of attaching the thread is
/// only seen once before we start profiling.
fn jni_new_string_with_repeat_permanent_thread_attachments(c: &mut Criterion) {
	// Create a permanent attachment before we start the benchmark
	VM.attach_current_thread(|_env| -> abos_jni::errors::Result<()> { Ok(()) })
		.expect("attached JNI callback succeeds");

	c.bench_function("jni_new_string_with_repeat_permanent_thread_attachments", |b| {
		b.iter(|| {
			VM.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
				black_box(
					JString::from_jni_str(env, jni_str!("Test"))
						.expect("test JNI string can be allocated")
						.auto(),
				);
				Ok(())
			})
			.expect("attached JNI callback succeeds");
		});
	});

	VM.detach_current_thread().expect("test thread detaches");
}

fn native_arc(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let class: &JNIStr = CLASS_OBJECT;
		let obj: JObject<'_> = env
			.new_object(class, &SIG_OBJECT_CTOR, &[])
			.expect("test object can be allocated");
		let global_ref: Global<JObject<'_>> = env.new_global_ref(&obj).expect("global reference can be allocated");
		env.delete_local_ref(obj);
		let arc: Arc<Global<JObject<'_>>> = Arc::new(global_ref);

		c.bench_function("native_arc", |b| {
			b.iter(|| {
				let _ = black_box(Arc::clone(&arc));
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

fn native_rc(c: &mut Criterion) {
	VM.attach_current_thread_for_scope(|env| -> abos_jni::errors::Result<()> {
		let rc: Rc<&mut abos_jni::Env<'_>> = Rc::new(env);

		c.bench_function("native_rc", |b| {
			b.iter(|| {
				let _ = black_box(Rc::clone(&rc));
			});
		});
		Ok(())
	})
	.expect("scoped JNI callback succeeds");
}

criterion_group!(
	benches,
	native_call_function,
	jni_call_static_abs_method_safe,
	jni_call_static_abs_method_unchecked_str,
	jni_call_static_abs_method_unchecked_jclass,
	jni_call_static_date_time_method_safe,
	jni_call_static_date_time_method_unchecked_jclass,
	jni_call_object_hash_method_safe,
	jni_call_object_hash_method_unchecked,
	jni_new_object_str,
	jni_new_object_by_id_str,
	jni_new_object_jclass,
	jni_new_object_by_id_jclass,
	jni_new_global_ref,
	jni_check_exception,
	jni_get_java_vm,
	jni_get_string,
	jni_noop_with_local_frame,
	jni_with_local_frame_returning_local,
	jni_with_local_frame_returning_global_to_local,
	jni_new_string_within_single_thread_attachment,
	jni_new_string_with_repeat_scoped_thread_attachments,
	jni_new_string_with_repeat_permanent_thread_attachments,
	native_arc,
	native_rc,
	strings::strings,
	arrays::arrays,
	comparisons::calls,
	reference_calls::reference_calls,
	comparisons::references,
	comparisons::attachment
);

criterion_main!(benches);
