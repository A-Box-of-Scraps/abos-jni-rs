# abos-jni

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jni-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jni)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)

Rust bindings and utilities for the Java Native Interface.

## Installation

Choose the dependency configuration for the direction of the call.

### Java calls Rust

For native methods loaded into an existing JVM, use the default features:

```toml
[dependencies]
abos-jni = "0.1.0"
```

Build the Rust library as a `cdylib`, as shown below. The Java application supplies the JVM; the Rust library does not
need the `invocation` feature.

### Rust calls Java

To start a desktop JVM from Rust, enable `invocation`:

```toml
[dependencies]
abos-jni = { version = "0.1.0", features = ["invocation"] }
```

The crate does not install or bundle Java. Starting a JVM requires an installed, compatible JVM shared library. Set
`JAVA_HOME` to its installation root. If `JAVA_HOME` is empty or unset, discovery uses the platform Java installation:
`/usr/libexec/java_home` on macOS, or `java` on `PATH` on other desktop platforms. A nonempty but invalid `JAVA_HOME` is
an error, not a request to fall back. `JavaVM::with_libjvm(...)` accepts an explicit library path.

On Android, use the application's existing VM. Starting a new VM through `JavaVM::new` is unsupported there.

## Quick Start

### Call Java from Rust

In a Rust binary crate with `invocation` enabled, put this in `src/main.rs`:

```rust
use abos_jni::vm::{InitArgsBuilder, JavaVM};
use abos_jni::{JNIVersion, JValue, jni_sig, jni_str};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = InitArgsBuilder::new()
        .version(JNIVersion::V1_8)
        .option("-Xcheck:jni")
        .build()?;
    let vm = JavaVM::new(args)?;

    vm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
        let result = env.call_static_method(
            jni_str!("java/lang/Math"),
            jni_str!("abs"),
            jni_sig!((jint) -> jint),
            &[JValue::Int(-10)],
        )?;
        assert_eq!(result.i()?, 10);
        Ok(())
    })?;

    Ok(())
}
```

Run it with `cargo run`. This uses the standard Java `Math.abs(int)` method, so no Java source compilation or custom
classpath is needed. `-Xcheck:jni` enables additional JVM checks useful during development.

### Call Rust from Java

This example needs a JDK with `javac` and `java`. Create a Rust library:

```sh
cargo new --lib hello-jni
cd hello-jni
```

Configure `Cargo.toml`:

```toml
[package]
name = "hello-jni"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
abos-jni = "0.1.0"
```

Replace `src/lib.rs` with:

```rust
use abos_jni::objects::JClass;
use abos_jni::sys::jint;
use abos_jni::{Env, NativeMethod, native_method};

const _: NativeMethod = native_method! {
    java_type = "com.example.HelloRust",
    static extern fn add(left: jint, right: jint) -> jint,
};

fn add(
    _env: &mut Env<'_>,
    _class: JClass<'_>,
    left: jint,
    right: jint,
) -> abos_jni::errors::Result<jint> {
    Ok(left.wrapping_add(right))
}
```

The macro exports the JNI symbol for `HelloRust.add` and wraps the Rust function with the native-call ABI and default
error/panic handling. No manual native-method registration is needed for this exported method.

Create `HelloRust.java` in the Cargo package root:

```java
package com.example;

public class HelloRust {
    static {
        System.loadLibrary("hello_jni");
    }

    public static native int add(int left, int right);

    public static void main(String[] args) {
        System.out.println(add(20, 22));
    }
}
```

Build and run from that directory:

```sh
cargo build
javac -d target/java HelloRust.java
java -Djava.library.path=target/debug -cp target/java com.example.HelloRust
```

The output is `42`. These commands assume Cargo's default target directory. The shared library is named
`libhello_jni.so` on Linux, `libhello_jni.dylib` on macOS, or `hello_jni.dll` on Windows; `System.loadLibrary` takes the
name without the platform prefix or extension.

## Core concepts

### Environments and threads

`JavaVM` is the handle to the VM; `Env` provides access to JNI operations on an attached thread. Obtain an `Env` through
closure-based APIs such as `JavaVM::attach_current_thread`, rather than constructing one yourself. Environment handles
and local references must not be sent to another thread; attach that thread separately.

`attach_current_thread` requests an attachment lasting until thread exit if a new attachment is needed.
`attach_current_thread_for_scope` instead detaches a newly attached thread when the callback returns. Neither changes
the lifetime of an existing attachment.

`Env` is not FFI-safe. A handwritten native entry point must receive `EnvUnowned` and use `with_env(...)` to obtain an
`Env`, then resolve its outcome with an error policy. The normal `native_method!` wrapper handles this for you.

See the [environment API](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/src/api/env/mod.rs)
and
[attachment API](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/src/api/vm/java_vm/attachment.rs).

### References and local frames

Types such as `JObject<'local>` and `JString<'local>` normally hold local references tied to a JNI frame. They cannot
outlive that frame. Ordinary local references are **not deleted just because their Rust wrappers are dropped**: the JVM
releases them when the frame ends. In long-running loops, use `with_local_frame(...)`, explicit deletion, or an `Auto`
wrapper to release references promptly.

Use `Global<T>` when an object must survive the current frame or be shared across threads. It keeps the Java object
alive until the global reference is dropped. A global reference does not make the object's Java operations inherently
thread-safe, and each thread still needs an attachment to call Java.

See the [reference APIs](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jni/src/api/refs).

### Exceptions and errors

A Rust `Err` does not necessarily mean a Java exception has been cleared. `Error::JavaException` indicates a pending
exception; `Error::CaughtJavaException` contains an exception that has been caught and cleared. Handle or propagate
pending exceptions deliberately instead of continuing with arbitrary JNI calls.

By default, attachment callbacks temporarily preserve pre-existing exceptions and restore them afterwards. A new
exception left pending by the callback is caught and returned as `Error::CaughtJavaException`. Use `AttachConfig` when
you need another attachment exception policy.

Normal native-method wrappers catch unwinding Rust panics and use `ThrowRuntimeExAndDefault` to translate Rust errors or
panics into Java `RuntimeException`s and default JNI return values. An already-pending Java exception takes precedence.
This cannot recover from `panic = "abort"`; raw wrappers or disabled panic catching require their own boundary handling.

See the
[error policies](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/src/api/errors/policy.rs).

### Class loading

Finding a class depends on the class loader, not just its name. Classes visible to Java application code may not be
visible through the default lookup on a Rust-created thread. Use `LoaderContext` to supply the intended loader when
initializing typed bindings or loading classes, especially with custom loaders or Android application classes.

`Env::find_class` takes slash-separated internal names such as `java/lang/String`. `LoaderContext::load_class` takes
dot-separated binary names such as `java.lang.String`.

See the
[loader API](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/src/api/refs/loader_context.rs).

### Checked and unchecked APIs

Start with checked calls such as `call_static_method` or typed bindings. Checked calls resolve names and signatures and
validate arguments. The `_unchecked` methods require the caller to uphold their safety contracts; incorrect IDs,
signatures, or arguments can cause undefined behavior. They are not a general-purpose shortcut around checked calls.

The crate assumes JNI 1.4 or later and one JVM per process. Destroying and recreating the VM is not a supported
workflow.

## Typed bindings and macros

Use `bind_java_type!` when calling the same Java API repeatedly. It generates a Rust wrapper and caches lookup metadata:

```rust
use abos_jni::sys::jint;
use abos_jni::{Env, bind_java_type};

bind_java_type! {
    JMath => "java.lang.Math",
    methods {
        static fn abs(value: jint) -> jint,
    }
}

fn absolute(env: &mut Env<'_>, value: jint) -> abos_jni::errors::Result<jint> {
    JMath::abs(env, value)
}
```

| Macro             | Purpose                                                                          |
| ----------------- | -------------------------------------------------------------------------------- |
| `bind_java_type!` | Define typed Java wrappers, constructors, methods, fields, and native interfaces |
| `native_method!`  | Wrap a Rust native implementation and optionally export its JNI symbol           |
| `jni_sig!`        | Parse typed JNI method or field signatures at compile time                       |
| `jni_str!`        | Encode a string literal as a modified UTF-8 JNI string                           |

Use `JNIString` for dynamically supplied JNI names and `JString` methods for Java string values. JNI's UTF-8 APIs use
modified UTF-8, not ordinary Rust UTF-8; use the string APIs instead of casting Rust string buffers to JNI pointers.

For automatic generation from Java sources, class files, JARs, or Android SDKs, see
[abos-jbindgen](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jbindgen/README.md).

## Examples and API reference

From a repository checkout, run an example with its required feature enabled:

```sh
cargo run --package abos-jni --features invocation --example bind_java_type
```

The JVM examples also compile Java fixtures and therefore require a JDK.

| Example                                                                                                                                             | Focus                                                |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| [bind_java_type](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/bind_java_type.rs)                               | Typed methods, fields, arrays, and native interfaces |
| [native_method](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/native_method.rs)                                 | Native wrappers, registration, and error policies    |
| [bind_java_type_is_instance_of](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/bind_java_type_is_instance_of.rs) | Inheritance and typed reference conversions          |
| [bind_java_type_hooks](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/bind_java_type_hooks.rs)                   | Binding initialization and class-loader hooks        |
| [jni_sig](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/jni_sig.rs)                                             | Signature syntax and output variants                 |
| [jni_str](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/jni_str.rs)                                             | Modified UTF-8 literals and conversions              |

Replace the example name in the command to run another entry. Build and open the full rustdoc reference with:

```sh
cargo doc --package abos-jni --features invocation --open
```

## Troubleshooting

| Symptom                                            | Check                                                                                                                                                             |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| JVM library cannot be found or loaded              | Check `JAVA_HOME`, process/JVM architecture, and the platform loader's dependency paths. Use `JavaVM::with_libjvm` for an explicit JVM library path.              |
| `ClassNotFoundException` or `NoClassDefFoundError` | Check the Java classpath, class-name format, and the class loader used by the calling thread.                                                                     |
| `UnsatisfiedLinkError`                             | Check `java.library.path`, the shared-library name and architecture, and that exported class/method names and signatures match the Java declarations.             |
| Mutable-borrow errors involving `env`              | Create reference arguments before the call rather than nesting another mutable `env` operation inside it. Shadow outer `env` variables when entering a new frame. |
| A local reference cannot escape a callback         | Return owned Rust data, a global reference, or use an API designed to return a local reference into the caller's frame.                                           |

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
