#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
//! Utilities for testing the `native_method!` macro.
//!
//! This module provides helper functions and macros to simplify writing tests for
//! the `native_method!` macro, particularly for runtime tests that load Java
//! fixtures, register native methods, and call them through JNI.

/// Load a test class from the output directory into the JVM.
///
/// This helper loads a compiled Java class file into the JVM using a system class loader.
/// The class should be in the package `com.example`.
#[allow(
	dead_code,
	reason = "Shared test and example helpers are used by different subsets of binaries."
)]
///
/// # Errors
///
/// Returns an error if the class cannot be defined in the selected class loader.
///
/// # Panics
///
/// Panics if the class file is missing or unreadable, or the system class loader cannot be obtained.
pub fn load_test_class<'local>(
	env: &mut abos_jni::Env<'local>,
	out_dir: &std::path::Path,
	class_name: &str,
) -> abos_jni::errors::Result<abos_jni::objects::JClass<'local>> {
	let class_path: std::path::PathBuf = out_dir.join(format!("com/example/{class_name}.class"));
	assert!(
		class_path.exists(),
		"{class_name}.class not found at {}",
		class_path.display()
	);

	let class_bytes: Vec<u8> =
		std::fs::read(&class_path).unwrap_or_else(|_| panic!("Failed to read {class_name}.class"));

	let class_loader: abos_jni::objects::JClassLoader<'_> =
		abos_jni::objects::JClassLoader::get_system_class_loader(env).expect("Failed to get system class loader");

	let class_internal_name: String = format!("com/example/{class_name}");
	let class_jni = abos_jni::strings::JNIString::new(class_internal_name.as_str());

	env.define_class(Some(&class_jni), &class_loader, &class_bytes)
}

/// Macro to simplify native method tests that manually register methods.
///
/// This macro reduces boilerplate for tests that need to:
/// 1. Reuse compiled Java fixtures
/// 2. Set up the JVM
/// 3. Load the class
/// 4. Register native methods
/// 5. Execute test code
/// 6. Optionally handle expected exceptions
///
/// # Examples
///
/// Basic test that expects success:
/// ```ignore
/// native_method_test! {
///     test_name: test_my_native_method,
///     java_class: "com/example/TestClass.java",
///     methods: |class| &[
///         native_method! {
///             name = "myMethod",
///             sig = (value: jint) -> jint,
///             fn = my_method_impl,
///         },
///     ],
///     test_body: |env, class| {
///         let result = env.call_static_method(class, "myMethod", "(I)I", &[abos_jni::objects::JValue::Int(42)])?;
///         assert_eq!(result.i()?, 84);
///         Ok(())
///     }
/// }
/// ```
///
/// Test that expects an exception:
/// ```ignore
/// native_method_test! {
///     test_name: test_expected_exception,
///     java_class: "com/example/TestClass.java",
///     methods: |class| &[
///         native_method! {
///             name = "badMethod",
///             sig = () -> void,
///             fn = bad_method_impl,
///         },
///     ],
///     expect_exception: "Method should throw",
///     test_body: |env, class| {
///         env.call_static_method(class, "badMethod", "()V", &[])?;
///         Ok(())
///     }
/// }
/// ```
#[macro_export]
macro_rules! native_method_test {
    (
        $(#[$attribute:meta])*
        test_name: $test_name:ident,
        java_class: $java_class:literal,
        methods: |$class_param:ident| $methods:expr,
        test_body: |$env:ident, $class:ident| $body:block
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
                    let $class = $crate::native_methods_utils::load_test_class($env, &out_dir, class_name)?;

                    let methods: &[abos_jni::NativeMethod] = $methods;
                    // SAFETY: The fixture class declares the names and JNI signatures supplied by these native-method descriptors.
                    unsafe {
                        $env.register_native_methods(&$class, methods)?;
                    }

                    $body
                })
                .expect(concat!(stringify!($test_name), " failed"));
            }
        }
    };

    (
        $(#[$attribute:meta])*
        test_name: $test_name:ident,
        java_class: $java_class:literal,
        methods: |$class_param:ident| $methods:expr,
        expect_exception: $expected_msg:expr,
        test_body: |$env:ident, $class:ident| $body:block
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
                    let $class = $crate::native_methods_utils::load_test_class($env, &out_dir, class_name)?;

                    let methods: &[abos_jni::NativeMethod] = $methods;
                    // SAFETY: The fixture class declares the names and JNI signatures supplied by these native-method descriptors.
                    unsafe {
                        $env.register_native_methods(&$class, methods)?;
                    }

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

/// Macro to call an instance method with integer argument.
///
/// Simplified wrapper around `call_method` for the common case of calling
/// a method that takes a single `jint` and returns a `jint`.
///
/// # Example
/// ```ignore
/// let result = call_int_method!(env, &obj, "callMethod", 42)?;
/// ```
#[macro_export]
macro_rules! call_int_method {
	($env:expr, $obj:expr, $method_name:literal, $arg:expr) => {{
		use abos_jni::{jni_sig, jni_str};
		let result = $env.call_method(
			$obj,
			jni_str!($method_name),
			jni_sig!("(I)I"),
			&[abos_jni::objects::JValue::Int($arg)],
		)?;
		result.i()
	}};
}

/// Macro to call a static method with integer argument.
///
/// Simplified wrapper around `call_static_method` for the common case of calling
/// a static method that takes a single `jint` and returns a `jint`.
///
/// # Example
/// ```ignore
/// let result = call_static_int_method!(env, class, "callStaticMethod", 42)?;
/// ```
#[macro_export]
macro_rules! call_static_int_method {
	($env:expr, $class:expr, $method_name:literal, $arg:expr) => {{
		use abos_jni::{jni_sig, jni_str};
		let result = $env.call_static_method(
			$class,
			jni_str!($method_name),
			jni_sig!("(I)I"),
			&[abos_jni::objects::JValue::Int($arg)],
		)?;
		result.i()
	}};
}

/// Macro to create a new object with default constructor.
///
/// Simplified wrapper around `new_object` for the common case of creating
/// an object with a no-argument constructor.
///
/// # Example
/// ```ignore
/// let obj = new_object!(env, class)?;
/// ```
#[macro_export]
macro_rules! new_object {
	($env:expr, $class:expr) => {{
		use abos_jni::jni_sig;
		$env.new_object(&$class, jni_sig!("()V"), &[])
	}};
}
