#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
// Based on the example from https://github.com/jni-rs/jni-rs/issues/539
// this checks the ergonomics of using get_rust_field to lock multiple fields
// without losing access to any `&mut Env` reference you might have.

use abos_jni::{EnvUnowned, jni_str, objects::JObject};

pub struct Renderer {
	// fields
}
impl Renderer {
	pub const fn render(&mut self, _surface: &Surface) {
		// rendering logic
	}
}

pub struct Surface {
	// fields
}

#[expect(
	clippy::significant_drop_tightening,
	reason = "Keep both field guards alive while exercising subsequent mutable Env access."
)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_Renderer_render<'local>(
	mut unowned_env: EnvUnowned<'local>,
	_this: JObject<'local>,
	renderer: JObject<'local>,
	surface: JObject<'local>,
) {
	unowned_env
		.with_env(|env| -> abos_jni::errors::Result<_> {
			// SAFETY: The native callback contract requires mNative to contain the Renderer installed with set_rust_field.
			let mut renderer: std::sync::MutexGuard<'_, Renderer> =
				unsafe { env.get_rust_field::<_, _, Renderer>(renderer, jni_str!("mNative"))? };
			// SAFETY: The native callback contract requires mNative to contain the Surface installed with set_rust_field.
			let surface: std::sync::MutexGuard<'_, Surface> =
				unsafe { env.get_rust_field::<_, _, Surface>(surface, jni_str!("mNative"))? };
			renderer.render(&surface);

			// Check we can still call something requiring `&mut Env` after locking multiple fields
			let _s: abos_jni::objects::JString<'_> = env.new_string("hello")?;

			Ok(())
		})
		.resolve::<abos_jni::errors::LogErrorAndDefault>();
}
