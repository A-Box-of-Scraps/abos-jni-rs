// Test that constructors cannot have a non-void return type (shorthand syntax)

use abos_jni::bind_java_type;

bind_java_type! {
    rust_type = TestClass,
    java_type = "com.example.TestClass",
    constructors = {
        fn new() -> jint
    }
}

fn main() {}
