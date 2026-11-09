#![allow(
	unsafe_code,
	reason = "Benchmarks exercise JVMTI on an attached thread in the live phase."
)]
#![allow(missing_docs, reason = "Benchmarks are not library API.")]
#![cfg(feature = "invocation")]

use std::sync::LazyLock;

use abos_jni::{InitArgsBuilder, JNIVersion, JavaVM};
use abos_jvmti::{Env, sys};
use criterion::{Criterion, criterion_group, criterion_main};

#[path = "api_calls/queries.rs"]
mod queries;
#[path = "api_calls/resources.rs"]
mod resources;

static VM: LazyLock<JavaVM> = LazyLock::new(|| {
	let args: abos_jni::InitArgs<'_> = InitArgsBuilder::new()
		.version(JNIVersion::V1_8)
		.build()
		.expect("benchmark JVM arguments are valid");
	JavaVM::new(args).expect("benchmark JVM starts")
});

fn with_jvmti(c: &mut Criterion, body: impl FnOnce(&mut Criterion, &Env<'_>)) {
	VM.attach_current_thread_for_scope(|_| -> abos_jni::errors::Result<()> {
		// SAFETY: The VM stays alive and this attached thread is in the live phase, outside callbacks.
		let env: Env<'_> = unsafe { Env::from_vm(&VM) }.expect("JVMTI environment is available");
		// SAFETY: The thread is attached and outside callbacks.
		let phase: sys::jvmtiPhase = unsafe { env.phase() }.expect("phase query succeeds");
		assert_eq!(phase, sys::JVMTI_PHASE_LIVE);
		body(c, &env);
		// SAFETY: Benchmarks release all resources before returning and register no callbacks.
		unsafe { env.dispose() }.expect("benchmark environment is disposed");
		Ok(())
	})
	.expect("scoped JNI attachment succeeds");
}

criterion_group!(benches, queries::queries, resources::resources);
criterion_main!(benches);
