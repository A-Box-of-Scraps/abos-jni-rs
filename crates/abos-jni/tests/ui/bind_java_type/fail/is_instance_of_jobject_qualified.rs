// Test that abos_jni::objects::JObject cannot be explicitly specified in is_instance_of block

abos_jni_macros::bind_java_type! {
    rust_type = TestClass,
    java_type = "com.example.TestClass",
    is_instance_of = {
        abos_jni::objects::JObject,
    }
}

fn main() {}
