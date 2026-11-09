use super::{assert_wrong_type, util};
use abos_jni::{
	JValue, bind_java_type,
	errors::Error,
	jni_sig, jni_str,
	objects::{JClassLoader, JObject},
	refs::LoaderContext,
};
use rusty_fork::rusty_fork_test;

bind_java_type! {
	CachedCalls => "com.example.ReferenceTypes",
	constructors {
		fn new(),
		fn with_comparable(value: "java.lang.Comparable"),
	},
	methods {
		fn accept_pair(ignored: jint, first: "java.lang.Comparable", second: "java.lang.Comparable") -> jint,
		static fn accept_static(value: "java.lang.Comparable") -> jint,
	}
}

bind_java_type! {
	CachedChild => "com.example.ReferenceTypes$Child",
	constructors { fn new(), },
	methods {
		fn echo(value: "com.example.ReferenceTypes") -> "com.example.ReferenceTypes",
	}
}

rusty_fork_test! {
#[test]
fn cached_validation_checks_arguments_that_were_initially_null() {
	let output = util::java_fixtures();
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let receiver = CachedCalls::new(env)?;
		let valid = env.new_string("valid")?;
		let invalid = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let null = JObject::null();
		// All-null calls must not prevent later metadata initialization. Initializing
		// with only the second reference non-null must still cache the first one's class.
		receiver.accept_pair(env, 0, &null, &null)?;
		receiver.accept_pair(env, 0, &null, &valid)?;
		for _ in 0..3 {
			assert_wrong_type(receiver.accept_pair(env, 0, &invalid, &valid), env);
			assert_wrong_type(receiver.accept_pair(env, 0, &valid, &invalid), env);
			receiver.accept_pair(env, 0, &valid, &null)?;
			CachedCalls::accept_static(env, &valid)?;
			assert_wrong_type(CachedCalls::accept_static(env, &invalid), env);
			CachedCalls::with_comparable(env, &valid)?;
			assert_wrong_type(CachedCalls::with_comparable(env, &invalid), env);
		}
		// Only valid invocations may execute their Java bodies.
		let class = env.get_object_class(&receiver)?;
		assert_eq!(env.get_static_field(&class, jni_str!("calls"), jni_sig!("I"))?.i()?, 11);
		Ok(())
	}).expect("warm caches retain reference checks");
}

#[test]
fn cached_validation_preserves_pending_exceptions_and_recovers() {
	let output = util::java_fixtures();
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let receiver = CachedCalls::new(env)?;
		let valid = env.new_string("valid")?;
		for _ in 0..2 {
			assert!(matches!(env.throw_new(jni_str!("java/lang/IllegalStateException"), jni_str!("pending")), Err(Error::JavaException)));
			assert!(matches!(receiver.accept_pair(env, 0, &valid, &valid), Err(Error::JavaException)));
			assert!(env.exception_check());
			env.exception_clear();
			receiver.accept_pair(env, 0, &valid, &valid)?;
		}
		Ok(())
	}).expect("cold and warm validation preserve pending exceptions");
}

#[test]
fn cached_inherited_parameters_keep_their_defining_loader() {
	let output = util::java_fixtures();
	let base_bytes = std::fs::read(output.join("com/example/ReferenceTypes.class")).expect("base fixture");
	let child_bytes = std::fs::read(output.join("com/example/ReferenceTypes$Child.class")).expect("child fixture");
	util::attach_current_thread(|env| {
		let urls = env.new_object_array(0, jni_str!("java/net/URL"), JObject::null())?;
		let mut loaders = Vec::new();
		let mut bases = Vec::new();
		for _ in 0..2 {
			let loader = env.new_object(
				jni_str!("java/net/URLClassLoader"),
				jni_sig!("([Ljava/net/URL;Ljava/lang/ClassLoader;)V"),
				&[JValue::Object(&urls), JValue::Object(&JObject::null())],
			)?;
			let loader = env.cast_local::<JClassLoader>(loader)?;
			bases.push(env.define_class(Some(jni_str!("com/example/ReferenceTypes")), &loader, &base_bytes)?);
			env.define_class(Some(jni_str!("com/example/ReferenceTypes$Child")), &loader, &child_bytes)?;
			loaders.push(loader);
		}
		CachedChildAPI::get(env, &LoaderContext::Loader(&loaders[0]))?;
		let receiver = CachedChild::new(env)?;
		let compatible = env.new_object(&bases[0], jni_sig!("()V"), &[])?;
		let incompatible = env.new_object(&bases[1], jni_sig!("()V"), &[])?;
		for _ in 0..3 {
			let returned = receiver.echo(env, &compatible)?;
			assert!(env.is_same_object(&compatible, &returned)?);
			env.delete_local_ref(returned);
			assert_wrong_type(receiver.echo(env, &incompatible), env);
			let returned = receiver.echo(env, &receiver)?;
			assert!(env.is_same_object(&receiver, &returned)?);
			env.delete_local_ref(returned);
		}
		Ok(())
	}).expect("cached classes preserve loader identity and inherited parameter types");
}
}
