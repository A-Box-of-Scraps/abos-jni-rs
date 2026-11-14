use abos_jni::{Env, errors::Result, elements::ReleaseMode};

fn escape(env: &Env) -> Result<()> {
	let guard = env.with_local_frame(1, |inner| {
		let array = inner.new_int_array(1)?;
		unsafe { array.into_elements_critical(inner, ReleaseMode::CopyBack) }
	})?;
	drop(guard);
	Ok(())
}

fn main() {}
