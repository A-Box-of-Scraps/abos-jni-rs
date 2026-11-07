use abos_jni::Env;
use abos_jni::errors::Error;
use abos_jni::objects::JObject;
use abos_jni::sys::{jint, jlong};
use abos_jni_macros::native_method;

// This should fail because the function takes jlong but signature expects jint

fn my_method_impl<'local>(
    _env: &mut Env<'local>,
    _this: JObject<'local>,
    _value: jlong, // Wrong! Should be jint
) -> Result<(), Error> {
    Ok(())
}

fn main() {
    let _method = native_method! {
        name = "myMethod",
        sig = (value: jint) -> void,
        fn = my_method_impl
    };
}
