use abos_jni::{Env, errors::Result, refs::IntoAuto as _};

fn move_to_thread(env: &mut Env<'_>) -> Result<()> {
	let string = env.new_string("local")?.auto();
	std::thread::scope(|scope| {
		scope.spawn(move || drop(string));
	});
	Ok(())
}

fn main() {}
