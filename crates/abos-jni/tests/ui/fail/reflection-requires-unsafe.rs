use abos_jni::{
	Env,
	ids::{JMethodID, JStaticMethodID},
	objects::JClass,
};

fn reflect(env: &mut Env<'_>, class: &JClass<'_>, instance: JMethodID, static_method: JStaticMethodID) {
	let _ = env.to_reflected_method_unchecked(class, instance);
	let _ = env.to_reflected_static_method_unchecked(class, static_method);
}

fn main() {}
