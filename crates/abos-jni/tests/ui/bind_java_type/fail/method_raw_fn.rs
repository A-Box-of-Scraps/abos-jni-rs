// Test that fn/raw cannot be used with regular methods

use abos_jni::EnvUnowned;
use abos_jni::bind_java_type;
use abos_jni::sys::jint;

extern "system" fn raw_method<'local>(_env: EnvUnowned<'local>, _this: TestClass<'local>) -> jint {
    42
}

bind_java_type! {
    rust_type = TestClass,
    java_type = "com.example.TestClass",
    methods = {
        fn test_method {
            sig = () -> jint,
            fn = raw_method,
            raw = true,
        }
    }
}

fn main() {}
