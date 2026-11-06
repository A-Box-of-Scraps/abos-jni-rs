use abos_jni::{Env, EnvUnowned, native_method};
use abos_jni::objects::JObject;
use abos_jni::sys::{jint, jobject};

fn implementation<'local>(_: &mut Env<'local>, _: JObject<'local>) -> abos_jni::errors::Result<jint> {
    Ok(42)
}

unsafe extern "system" {
    fn custom_named_export(env: EnvUnowned<'_>, this: jobject) -> jint;
}

fn main() {
    let _method = native_method! {
        name = "method",
        sig = () -> jint,
        fn = implementation,
        export = "custom_named_export",
    };
    // Referencing the symbol checks that the wrapper links under its explicit name.
    let _export = std::hint::black_box(custom_named_export as unsafe extern "system" fn(EnvUnowned<'_>, jobject) -> jint);
}
