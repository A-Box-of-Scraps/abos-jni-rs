#[path = "../../util/mod.rs"]
mod util;
use util::attach_current_thread;

use abos_jni::objects::JString;
use abos_jni::Env;

pub fn main() {
    attach_current_thread(|env0: &mut Env| {
        env0.with_local_frame(10, |_env1: &mut Env| -> abos_jni::errors::Result<_> {
            let _s = JString::from_str(env0, "hello").unwrap();
            eprintln!("BUG: this shouldn't compile since env0 shouldn't be mutable and new_string requires a mutable Env");
            Ok(())
        })
        .unwrap();
        Ok(())
    })
    .unwrap();
}
