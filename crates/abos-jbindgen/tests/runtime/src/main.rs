//! Execute generated bindings against a real JVM.
#[allow(dead_code, non_snake_case)]
#[expect(
	non_camel_case_types,
	reason = "Keyword-normalization fixtures intentionally generate match_ and match_API types."
)]
mod bindings {
	include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
#[allow(dead_code, non_snake_case)]
#[expect(
	non_camel_case_types,
	reason = "File-tree fixtures preserve the same keyword-normalized type names as single-file output."
)]
mod tree {
	include!(concat!(env!("OUT_DIR"), "/tree/mod.rs"));
}

#[allow(dead_code, non_snake_case)]
mod bytecode {
	include!(concat!(env!("OUT_DIR"), "/bytecode.rs"));
}
#[allow(dead_code, non_snake_case)]
#[expect(
	non_camel_case_types,
	reason = "The explicit Rust type-name override tests normalization of type to type_ and type_API."
)]
mod overrides {
	include!(concat!(env!("OUT_DIR"), "/overrides.rs"));
}

#[allow(dead_code, non_snake_case)]
mod collision_overrides {
	include!(concat!(env!("OUT_DIR"), "/collision_overrides.rs"));
}
#[allow(dead_code, non_snake_case)]
mod native_collisions {
	include!(concat!(env!("OUT_DIR"), "/native_collisions.rs"));
}
#[allow(dead_code, non_snake_case)]
mod modular_jar {
	include!(concat!(env!("OUT_DIR"), "/modular_jar.rs"));
}
#[allow(dead_code, non_snake_case)]
mod multi_release_jar {
	include!(concat!(env!("OUT_DIR"), "/multi_release_jar.rs"));
}
#[allow(dead_code, non_snake_case)]
mod reversed_jar {
	include!(concat!(env!("OUT_DIR"), "/reversed_jar.rs"));
}
mod collisions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	use bindings::com::example::SimpleClass;
	let args = abos_jni::InitArgsBuilder::new()
		.version(abos_jni::JNIVersion::V1_8)
		.option(format!("-Djava.class.path={}", env!("FIXTURE_CLASSES")))
		.build()?;
	let vm = abos_jni::JavaVM::new(args)?;
	vm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		bindings::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
		tree::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
		let instance = SimpleClass::new(env)?;
		instance.set_value(env, 42)?;
		assert_eq!(instance.get_value(env)?, 42);
		assert_eq!(SimpleClass::add(env, 20, 22)?, 42);
		let fields = bindings::com::example::WithFields::new(env)?;
		fields.set_public_field_(env, 17)?;
		assert_eq!(fields.public_field(env)?, 17);
		let collisions = bindings::com::example::AccessorCollisions::new(env)?;
		collisions.set_value_(env, 9)?;
		assert_eq!(collisions.value_(env)?, 9);
		let identifiers = bindings::com::type_::Identifiers::new(env)?;
		identifiers.set_type_(env, 11)?;
		identifiers.set_type__(env, 12)?;
		identifiers.set_non_null_(env, 13)?;
		identifiers.set_caf_ue9_(env, 14)?;
		assert_eq!(identifiers.type_(env)?, 11);
		assert_eq!(identifiers.type__(env)?, 12);
		assert_eq!(identifiers.non_null_(env)?, 13);
		assert_eq!(identifiers.caf_ue9_(env)?, 14);
		assert_eq!(identifiers.match_(env, 1, 2, 3, 4, 5)?, 54321);
		assert_eq!(identifiers.gen_(env)?, 7);
		assert_eq!(identifiers.self_(env)?, 8);
		assert_eq!(identifiers.dollars(env, 9)?, 9);
		assert_eq!(identifiers._u3bb_(env)?, 10);
		let keyword_type = bindings::com::type_::match_::new(env)?;
		assert_eq!(identifiers.echo(env, keyword_type)?.value(env)?, 42);
		let from_bytecode = bytecode::com::type_::Identifiers::new(env)?;
		assert_eq!(from_bytecode.match_(env, 1, 2, 3, 4, 5)?, 54321);
		let renamed = overrides::com::example::type_::gen_(env)?;
		renamed.set_value(env, 42)?;
		assert_eq!(renamed.with_dash(env)?, 42);
		collisions::check(env)?;
		assert_eq!(modular_jar::fixture::archive::ModularExample::answer(env)?, 42);
		// Binding generation uses base classes; the JVM still selects its runtime variant.
		assert_eq!(multi_release_jar::fixture::archive::VersionedExample::version(env)?, 17);
		assert_eq!(reversed_jar::fixture::archive::VersionedExample::version(env)?, 17);
		Ok(())
	})?;
	Ok(())
}
