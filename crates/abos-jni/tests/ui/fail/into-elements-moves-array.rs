use abos_jni::{Env, errors::Result, elements::ReleaseMode, objects::JIntArray};

fn regular(env: &Env, array: JIntArray) -> Result<()> {
	let guard = unsafe { array.into_elements(env, ReleaseMode::CopyBack)? };
	drop(guard);
	drop(array);
	Ok(())
}

fn critical(env: &Env, array: JIntArray) -> Result<()> {
	let guard = unsafe { array.into_elements_critical(env, ReleaseMode::CopyBack)? };
	drop(guard);
	drop(array);
	Ok(())
}

fn main() {}
