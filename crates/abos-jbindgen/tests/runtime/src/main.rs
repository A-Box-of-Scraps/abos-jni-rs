//! Execute generated bindings against a real JVM.
#[allow(dead_code, non_snake_case)]
mod bindings {
	include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
#[allow(dead_code, non_snake_case)]
mod tree {
	include!(concat!(env!("OUT_DIR"), "/tree/mod.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	use bindings::com::example::SimpleClass;
	let args = jni::InitArgsBuilder::new()
		.version(jni::JNIVersion::V1_8)
		.option(format!("-Djava.class.path={}", env!("FIXTURE_CLASSES")))
		.build()?;
	let vm = jni::JavaVM::new(args)?;
	vm.attach_current_thread(|env| -> jni::errors::Result<()> {
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
		Ok(())
	})?;
	Ok(())
}
