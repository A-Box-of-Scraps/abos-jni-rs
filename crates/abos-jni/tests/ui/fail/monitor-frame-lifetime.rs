use abos_jni::{Env, errors::Result, jni_sig, jni_str};

fn escape(env: &Env) -> Result<()> {
	let guard = env.with_local_frame(1, |inner| {
		let object = inner.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		env.lock_obj(&object)
	})?;
	drop(guard);
	Ok(())
}

fn main() {}
