use abos_jni::{Env, errors::Result, refs::Reference};

fn share_with_thread(env: &mut Env<'_>) -> Result<()> {
	let string = env.new_string("local")?;
	let borrowed = &string;
	std::thread::scope(|scope| {
		scope.spawn(move || assert!(!borrowed.is_null()));
	});
	Ok(())
}

fn main() {}
