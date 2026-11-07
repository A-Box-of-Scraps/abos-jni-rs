use abos_jni::EnvUnowned;
use abos_jni::objects::JObject;
use abos_jni::sys::jint;
use abos_jni_macros::native_method;

// This should fail because the raw function has 1 param but signature expects 2

extern "system" fn my_method_impl<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
    _value: jint, // Missing second parameter!
) {
}

fn main() {
    let _method = native_method! {
        name = "myMethod",
        sig = (a: jint, b: jint) -> void,
        fn = my_method_impl,
        raw = true,
    };
}
