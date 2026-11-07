use abos_jni::{JavaVM, errors::Result, objects::JObject, refs::Global};

fn escape(vm: &JavaVM, object: &'static Global<JObject<'static>>) -> Result<()> {
	let guard = vm.attach_current_thread_for_scope(|env| env.lock_obj(object))?;
	drop(guard);
	Ok(())
}

fn main() {}
