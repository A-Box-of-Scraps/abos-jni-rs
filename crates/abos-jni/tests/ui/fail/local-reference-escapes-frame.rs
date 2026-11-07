use abos_jni::{Env, errors::Result};

fn escape(env: &mut Env<'_>) -> Result<()> {
	let string = env.with_local_frame(1, |inner| inner.new_string("local"))?;
	string.try_to_string(env)?;
	Ok(())
}

fn main() {}
