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
#[allow(dead_code, non_snake_case)]
mod default_bytecode {
	include!(concat!(env!("OUT_DIR"), "/default_bytecode.rs"));
}
#[allow(dead_code, non_snake_case)]
mod default_unmapped {
	include!(concat!(env!("OUT_DIR"), "/default_unmapped.rs"));
}
#[allow(dead_code, non_snake_case)]
mod generated_members {
	include!(concat!(env!("OUT_DIR"), "/generated_members.rs"));
}
#[allow(dead_code, non_snake_case)]
mod generated_natives {
	include!(concat!(env!("OUT_DIR"), "/generated_natives.rs"));
}
#[allow(dead_code, non_snake_case, non_camel_case_types)]
mod helper_no_init {
	include!(concat!(env!("OUT_DIR"), "/helper_no_init.rs"));
}
#[allow(dead_code, non_snake_case, non_camel_case_types)]
mod parity_bytecode {
	include!(concat!(env!("OUT_DIR"), "/parity_bytecode.rs"));
}
mod collisions;
mod default_package;
mod enclosing;
mod enums;
mod generated_collisions;
mod generics;
mod parity;
mod records;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	use bindings::com::example::SimpleClass;
	let args = abos_jni::InitArgsBuilder::new()
		.version(abos_jni::JNIVersion::V1_8)
		.option("-Xcheck:jni")
		.option(format!("-Djava.class.path={}", env!("FIXTURE_CLASSES")))
		.build()?;
	let vm = abos_jni::JavaVM::new(args)?;
	vm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
		let scenario = std::env::args().nth(1).unwrap_or_else(|| "all".to_owned());
		eprintln!("runtime scenario: {scenario}");
		bindings::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
		parity_bytecode::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
		match scenario.as_str() {
			"generics" => return generics::check(env),
			"enclosing" => return enclosing::check(env),
			"records" => return records::check(env),
			"enums" => return enums::check(env),
			"members" => return parity::members(env),
			"inheritance" => return parity::inheritance(env),
			"identifiers" => return parity::identifiers(env),
			"existing" | "all" => {}
			_ => panic!("unknown runtime scenario: {scenario}"),
		}
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
		let dollars = bindings::com::example::UnsupportedNames::new(env)?;
		dollars.set__value(env, 42)?;
		assert_eq!(dollars._value(env)?, 42);
		assert_eq!(dollars.foo_bar(env)?, 42);
		let tree_dollars = tree::com::example::UnsupportedNames::new(env)?;
		tree_dollars.set__value(env, 17)?;
		assert_eq!(tree_dollars._value(env)?, 17);
		assert_eq!(tree_dollars.foo_bar(env)?, 17);
		collisions::check(env)?;
		generated_collisions::check(env)?;
		default_package::check(env)?;
		if scenario == "all" {
			generics::check(env)?;
			enclosing::check(env)?;
			records::check(env)?;
			enums::check(env)?;
			parity::members(env)?;
			parity::inheritance(env)?;
			parity::identifiers(env)?;
		}
		assert_eq!(modular_jar::fixture::archive::ModularExample::answer(env)?, 42);
		// Binding generation uses base classes; the JVM still selects its runtime variant.
		assert_eq!(multi_release_jar::fixture::archive::VersionedExample::version(env)?, 17);
		assert_eq!(reversed_jar::fixture::archive::VersionedExample::version(env)?, 17);
		Ok(())
	})?;
	Ok(())
}
