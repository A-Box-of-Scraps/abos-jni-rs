#![cfg(feature = "invocation")]
#![allow(
	dead_code,
	reason = "Shared test and example helpers are used by different subsets of binaries."
)]

use std::path::{Path, PathBuf};
use std::sync::Once;

use abos_jni::{Env, InitArgsBuilder, JNIVersion, JavaVM};

pub fn jvm() -> JavaVM {
	static INIT: Once = Once::new();
	INIT.call_once(|| {
		let jvm_args: abos_jni::InitArgs<'_> = InitArgsBuilder::new()
			.version(JNIVersion::V1_8)
			.option("-Xcheck:jni")
			.build()
			.unwrap_or_else(|e| panic!("{e:#?}"));

		let _jvm: JavaVM = JavaVM::new(jvm_args).unwrap_or_else(|e| panic!("{e:#?}"));
	});
	JavaVM::singleton().expect("Failed to get singleton JVM")
}

pub fn attach_current_thread<F, T>(callback: F) -> abos_jni::errors::Result<T>
where
	F: FnOnce(&mut Env) -> abos_jni::errors::Result<T>,
{
	jvm().attach_current_thread(|env| callback(env))
}

fn setup_javac_output_dir() -> PathBuf {
	let build_dir: PathBuf =
		option_env!("JAVA_BUILD_DIR").map_or_else(|| java_target_dir().join("examples-java-classes"), PathBuf::from);

	std::fs::create_dir_all(&build_dir).unwrap_or_else(|e| panic!("Failed to create javac output directory: {e}"));

	build_dir
}

fn java_target_dir() -> PathBuf {
	if let Some(target) = option_env!("CARGO_TARGET_DIR") {
		return PathBuf::from(target);
	}
	let Some(output) = option_env!("OUT_DIR") else {
		return PathBuf::from("target");
	};
	target_dir_from_output(Path::new(output))
}

pub fn target_dir_from_output(output: &Path) -> PathBuf {
	// Restored target directories need not contain Cargo's optional CACHEDIR.TAG.
	output
		.ancestors()
		.find(|path| path.file_name().is_some_and(|name| name == "target"))
		.map_or_else(|| PathBuf::from("target"), Path::to_path_buf)
}

fn jni_macros_dir() -> PathBuf {
	let manifest_dir: String = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set in environment");
	PathBuf::from(manifest_dir)
}

pub fn compile_class(name: &str) -> Vec<PathBuf> {
	let out_dir: PathBuf = setup_javac_output_dir();
	let jni_macros_dir: PathBuf = jni_macros_dir();

	let path: PathBuf = jni_macros_dir
		.join("examples/java/com/example")
		.join(name)
		.with_extension("java");
	let files: Vec<PathBuf> = abos_javac::Build::new()
		.file(path)
		.output_dir(&out_dir)
		.cargo_metadata(false) // don't output build.rs lines like "cargo:rerun-if-changed=..."
		.compile();

	println!("Compiled Java class {name}.java as: {files:?}");
	files
}

pub fn define_class(env: &mut Env, class_path: &Path) -> abos_jni::errors::Result<()> {
	use abos_jni::strings::JNIStr;

	let class_bytes: Vec<u8> =
		std::fs::read(class_path).unwrap_or_else(|_| panic!("Failed to read class file: {}", class_path.display()));

	let class_loader: abos_jni::objects::JClassLoader<'_> =
		abos_jni::objects::JClassLoader::get_system_class_loader(env)?;

	env.define_class(Option::<&JNIStr>::None, &class_loader, &class_bytes)?;

	Ok(())
}

pub fn load_class(env: &mut Env, name: &str) -> abos_jni::errors::Result<()> {
	let files: Vec<PathBuf> = compile_class(name);
	for class_file in &files {
		define_class(env, class_file)?;
	}
	Ok(())
}

/// Define a stub Reference type for use in examples, without depending on
/// the `bind_java_type`! macro.
#[macro_export]
macro_rules! define_stub_type {
	($rust_type:ident, $java_type:literal) => {
		#[repr(transparent)]
		#[derive(Default)]
		struct $rust_type<'local>(JObject<'local>);
		impl<'local> AsRef<JObject<'local>> for $rust_type<'local> {
			fn as_ref(&self) -> &JObject<'local> {
				&self.0
			}
		}
		impl<'local> From<$rust_type<'local>> for JObject<'local> {
			fn from(value: $rust_type<'local>) -> Self {
				value.0
			}
		}
		// SAFETY: The wrapper preserves JObject reference identity and delegates construction to its local/global constructors.
		unsafe impl Reference for $rust_type<'_> {
			type Kind<'local> = $rust_type<'local>;
			type GlobalKind = $rust_type<'static>;

			fn as_raw(&self) -> jni2::sys::jobject {
				self.0.as_raw()
			}

			fn class_name() -> std::borrow::Cow<'static, jni2::strings::JNIStr> {
				std::borrow::Cow::Borrowed(abos_jni::jni_str!($java_type))
			}

			fn lookup_class<'caller>(
				env: &Env<'_>,
				loader_context: &jni2::refs::LoaderContext,
			) -> jni2::errors::Result<impl std::ops::Deref<Target = jni2::refs::Global<JClass<'static>>> + 'caller> {
				static CLASS: std::sync::OnceLock<abos_jni::objects::Global<abos_jni::objects::JClass>> =
					std::sync::OnceLock::new();

				let class = if let Some(class) = CLASS.get() {
					class
				} else {
					env.with_local_frame(4, |env| -> abos_jni::errors::Result<_> {
						let class: abos_jni::objects::JClass =
							loader_context.load_class_for_type::<Self>(env, false)?;
						let global_class = env.new_global_ref(&class)?;
						drop(CLASS.set(global_class));
						Ok(CLASS.get().unwrap())
					})?
				};

				Ok(class)
			}

			unsafe fn kind_from_raw<'env>(reference: jni2::sys::jobject) -> Self::Kind<'env> {
				// SAFETY: The caller supplies a valid local reference of the declared Java type and lifetime.
				unsafe { $rust_type(JObject::kind_from_raw(reference)) }
			}

			unsafe fn global_kind_from_raw(global_ref: jni2::sys::jobject) -> Self::GlobalKind {
				// SAFETY: The caller supplies a valid global reference of the declared Java type.
				unsafe { $rust_type(JObject::global_kind_from_raw(global_ref)) }
			}
		}
		impl<'local> $rust_type<'local> {
			#[allow(
				dead_code,
				reason = "Shared test and example helpers are used by different subsets of binaries."
			)]
			pub fn new(env: &mut Env<'local>) -> jni2::errors::Result<Self> {
				let class = Self::lookup_class(env, &jni2::refs::LoaderContext::default())?;
				let class: &JClass = class.as_ref();
				let obj = env.new_object(class, abos_jni::jni_sig!("()V"), &[])?;
				Ok(Self(obj))
			}
		}
	};
}
