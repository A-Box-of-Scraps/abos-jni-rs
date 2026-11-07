use abos_jni::{Env, errors::Result};

fn move_to_thread(env: &mut Env<'_>) -> Result<()> {
	let string = env.new_string("local")?;
	std::thread::scope(|scope| {
		scope.spawn(move || drop(string));
	});
	Ok(())
}

fn main() {}
