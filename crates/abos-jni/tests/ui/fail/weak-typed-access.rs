use abos_jni::{Env, objects::JString, refs::Weak};

fn read(env: &Env, weak: &Weak<JString<'static>>) {
	let _: &JString = weak;
	let _: &JString = weak.as_ref();
	let _ = weak.mutf8_chars(env);
}

fn main() {}
