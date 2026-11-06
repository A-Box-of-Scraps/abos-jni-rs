use abos_jni::{
	Env,
	objects::{JObject, JString},
	refs::Weak,
};

fn inspect(env: &mut Env, weak: &Weak<JObject<'static>>) {
	let _ = env.get_object_class(weak);
	let _ = env.new_cast_local_ref::<JString>(weak);
	let _ = env.new_cast_global_ref::<JString>(weak);
}

fn main() {}
