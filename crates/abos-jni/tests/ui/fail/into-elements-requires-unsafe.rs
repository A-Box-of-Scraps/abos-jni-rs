use abos_jni::{Env, errors::Result, elements::ReleaseMode, objects::JIntArray};

fn regular(env: &Env, array: JIntArray) -> Result<()> {
	let guard = array.into_elements(env, ReleaseMode::CopyBack)?;
	drop(guard);
	Ok(())
}

fn critical(env: &Env, array: JIntArray) -> Result<()> {
	let guard = array.into_elements_critical(env, ReleaseMode::CopyBack)?;
	drop(guard);
	Ok(())
}

fn main() {}
