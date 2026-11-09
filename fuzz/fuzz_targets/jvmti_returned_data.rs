#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| abos_jni_fuzz::jvmti_returned_data::run(data));
