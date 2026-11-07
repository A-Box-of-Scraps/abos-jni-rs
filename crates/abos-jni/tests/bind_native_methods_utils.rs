#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
/// Macro to simplify native method tests.
#[macro_export]
macro_rules! native_method_test {
    (
        $(#[$attribute:meta])*
        test_name: $test_name:ident,
        java_class: $java_class:literal,
        api: $api:ty,
        test_body: |$env:ident| $body:block
    ) => {
        rusty_fork::rusty_fork_test! {
            #[test]
            $(#[$attribute])*
            fn $test_name() {
                #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                let out_dir = $crate::util::java_fixtures();

                #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                $crate::util::attach_current_thread(|$env| {
                    let class_name = $java_class
                        .trim_end_matches(".java")
                        .split('/')
                        .last()
                        .expect("Invalid Java class path");

                    #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                    $crate::util::load_test_class($env, &out_dir, class_name)?;
                    let loader = abos_jni::refs::LoaderContext::default();
                    <$api>::get($env, &loader)?;

                    $body
                })
                .expect(concat!(stringify!($test_name), " failed"));
            }
        }
    };

    // Variant that expects the test to fail with a JavaException
    (
        $(#[$attribute:meta])*
        test_name: $test_name:ident,
        java_class: $java_class:literal,
        api: $api:ty,
        expect_exception: $expected_msg:expr,
        test_body: |$env:ident| $body:block
    ) => {
        rusty_fork::rusty_fork_test! {
            #[test]
            $(#[$attribute])*
            fn $test_name() {
                #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                let out_dir = $crate::util::java_fixtures();

                #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                let result = $crate::util::attach_current_thread(|$env| {
                    let class_name = $java_class
                        .trim_end_matches(".java")
                        .split('/')
                        .last()
                        .expect("Invalid Java class path");

                    #[allow(clippy::crate_in_macro_def, reason = "The test helper resolves the utility module in the invoking test crate.")]
                    $crate::util::load_test_class($env, &out_dir, class_name)?;
                    let loader = abos_jni::refs::LoaderContext::default();
                    <$api>::get($env, &loader)?;

                    $body
                });

                match result {
                    Err(abos_jni::errors::Error::CaughtJavaException { msg, .. }) => {
                        assert_eq!(msg, $expected_msg);
                    }
                    _ => panic!("Expected CaughtJavaException: {}, got: {:?}", $expected_msg, result),
                }
            }
        }
    };
}
