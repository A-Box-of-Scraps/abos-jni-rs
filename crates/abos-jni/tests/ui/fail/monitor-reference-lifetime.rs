use abos_jni::{
	Env,
	errors::Result,
	objects::JObject,
	refs::{Auto, Global},
};

fn global(env: &Env, object: Global<JObject<'static>>) -> Result<()> {
	let guard = env.lock_obj(&object)?;
	drop(object);
	drop(guard);
	Ok(())
}

fn local<'local>(env: &Env<'local>, object: JObject<'local>) -> Result<()> {
	let guard = env.lock_obj(&object)?;
	env.delete_local_ref(object);
	drop(guard);
	Ok(())
}

fn auto<'local>(env: &Env<'local>, object: Auto<'local, JObject<'local>>) -> Result<()> {
	let guard = env.lock_obj(&object)?;
	drop(object);
	drop(guard);
	Ok(())
}

fn main() {}
