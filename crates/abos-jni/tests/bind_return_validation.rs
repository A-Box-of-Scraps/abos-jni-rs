#![allow(unsafe_code, reason = "Binding macros generate JNI calls.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env, JValue, bind_java_type,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClassLoader, JObject, JObjectArray},
	refs::{LoaderContext, Reference},
	strings::JNIString,
};
use assert_matches::assert_matches;
use rusty_fork::rusty_fork_test;

bind_java_type! {
	Value => "com.example.LoaderReturns$Value",
	methods {
		fn answer() -> jint,
	}
}

bind_java_type! {
	LoaderReturns => "com.example.LoaderReturns",
	type_map = {
		Value => "com.example.LoaderReturns$Value",
	},
	constructors {
		fn new(empty: bool),
	},
	methods {
		fn get_value() -> Value,
		fn get_values() -> Value[],
		fn get_matrix() -> Value[][],
		static fn get_static_value() -> Value,
		static fn get_static_values() -> Value[],
		static fn get_static_matrix() -> Value[][],
		non_null fn required_value {
			name = "getValue",
			sig = () -> Value,
		},
	},
	fields {
		value: Value,
		values: Value[],
		matrix: Value[][],
		static static_value: Value,
		static static_values: Value[],
		static static_matrix: Value[][],
		non_null required_field {
			name = "value",
			sig = Value,
		},
	}
}

fn load_fixture<'local>(env: &mut Env<'local>) -> Result<JClassLoader<'local>> {
	let output: std::path::PathBuf = util::java_fixtures();
	let urls: JObjectArray<'_> = env.new_object_array(0, jni_str!("java/net/URL"), JObject::null())?;
	let loader: JObject<'_> = env.new_object(
		jni_str!("java/net/URLClassLoader"),
		jni_sig!("([Ljava/net/URL;Ljava/lang/ClassLoader;)V"),
		&[JValue::Object(&urls), JValue::Object(&JObject::null())],
	)?;
	let loader: JClassLoader<'_> = env.cast_local::<JClassLoader>(loader)?;
	for name in ["LoaderReturns$Value", "LoaderReturns$Child", "LoaderReturns"] {
		let bytes: Vec<u8> = std::fs::read(output.join(format!("com/example/{name}.class"))).expect("fixture class");
		let name = JNIString::new(format!("com/example/{name}"));
		env.define_class(Some(&name), &loader, &bytes)?;
	}
	Ok(loader)
}

fn check_required_returns(
	env: &mut Env<'_>,
	source: &LoaderReturns<'_>,
	different_loader: bool,
	empty: bool,
) -> Result<()> {
	for result in [source.required_value(env), source.required_field(env)] {
		if empty {
			assert_matches!(result, Err(Error::NullPtr(_)));
		} else if different_loader {
			assert_matches!(result, Err(Error::WrongObjectType));
		} else {
			assert_eq!(result?.answer(env)?, 42);
		}
	}
	Ok(())
}

fn check_returns(different_loader: bool, empty: bool) {
	util::attach_current_thread(|env| {
		let source_loader: JClassLoader<'_> = load_fixture(env)?;
		let target_loader: JClassLoader<'_> = if different_loader {
			load_fixture(env)?
		} else {
			env.new_local_ref(&source_loader)?
		};
		// Each Rust wrapper caches a class independently, even when mapped names match.
		ValueAPI::get(env, &LoaderContext::Loader(&target_loader))?;
		LoaderReturnsAPI::get(env, &LoaderContext::Loader(&source_loader))?;
		let source: LoaderReturns<'_> = LoaderReturns::new(env, empty)?;

		let results: [Result<JObject<'_>>; 12] = [
			source.get_value(env).map(Into::into),
			source.get_values(env).map(Into::into),
			source.get_matrix(env).map(Into::into),
			LoaderReturns::get_static_value(env).map(Into::into),
			LoaderReturns::get_static_values(env).map(Into::into),
			LoaderReturns::get_static_matrix(env).map(Into::into),
			source.value(env).map(Into::into),
			source.values(env).map(Into::into),
			source.matrix(env).map(Into::into),
			LoaderReturns::static_value(env).map(Into::into),
			LoaderReturns::static_values(env).map(Into::into),
			LoaderReturns::static_matrix(env).map(Into::into),
		];
		for (index, result) in results.into_iter().enumerate() {
			if different_loader && !empty {
				assert_matches!(result, Err(Error::WrongObjectType), "return path {index}");
			} else {
				assert_eq!(result?.is_null(), empty);
			}
		}

		check_required_returns(env, &source, different_loader, empty)?;

		if !different_loader && !empty {
			// Subclasses and covariant arrays remain usable through generated methods.
			assert_eq!(source.get_value(env)?.answer(env)?, 42);
			assert_eq!(source.get_values(env)?.get_element(env, 0)?.answer(env)?, 42);
			assert_eq!(
				source
					.get_matrix(env)?
					.get_element(env, 0)?
					.get_element(env, 0)?
					.answer(env)?,
				42
			);
		}
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("generated returns preserve resolved class identity");
}

// Isolate the JVM and the generated wrappers' process-wide class caches.
rusty_fork_test! {
#[test]
fn rejects_returns_from_a_different_loader() {
	check_returns(true, false);
}

#[test]
fn accepts_subclasses_and_covariant_arrays() {
	check_returns(false, false);
}

#[test]
fn preserves_nullability_across_loaders() {
	check_returns(true, true);
}
}
