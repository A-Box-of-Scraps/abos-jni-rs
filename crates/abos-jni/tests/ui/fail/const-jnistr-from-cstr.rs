use abos_jni::strings::JNIStr;

fn main() {
    const INVALID_MUTF8_CSTR: &JNIStr = JNIStr::from_cstr(c"invalid mutf8: 🦀").unwrap();
}
