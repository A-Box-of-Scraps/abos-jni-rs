# abos-jni-macros

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jni--macros-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jni-macros)
[![crates.io](https://img.shields.io/crates/v/abos-jni-macros?style=for-the-badge&labelColor=555555&color=00599C)](https://crates.io/crates/abos-jni-macros)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Procedural macros for abos-jni.

## Installation

Most applications should use the macros re-exported by `abos-jni`:

```toml
[dependencies]
abos-jni = "0.1.0"
```

Import them from `abos_jni`, for example `use abos_jni::{jni_sig, jni_str};`. A separate dependency on this proc-macro
crate is not required. When importing from `abos_jni_macros` directly, add `abos-jni-macros = "0.1.0"` as well;
expansions that use JNI runtime types still need `abos-jni`.

The macros do not require a running JVM during compilation. Starting a desktop JVM from the resulting program requires
`abos-jni`'s `invocation` feature; calling into an existing JVM does not. See the
[abos-jni README](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/README.md) for complete
runtime setup and examples in both call directions.

## Quick Start

Build a JNI string and a typed method signature at compile time:

```rust,ignore
use abos_jni::{jni_sig, jni_str};

fn main() {
    let class_name = jni_str!("java/lang/String");
    let signature = jni_sig!((value: jint) -> java.lang.String);

    assert_eq!(class_name.to_bytes(), b"java/lang/String");
    assert_eq!(signature.sig().to_bytes(), b"(I)Ljava/lang/String;");
}
```

This example runs without a JVM or the `invocation` feature. The macros construct names and descriptors; they do not
look up Java classes or verify that a matching Java method exists.

## Macros

### Strings and signatures

| Macro           | Output                                               |
| --------------- | ---------------------------------------------------- |
| `jni_str!`      | `&'static JNIStr` containing a modified UTF-8 string |
| `jni_cstr!`     | `&'static CStr` containing a modified UTF-8 string   |
| `jni_sig!`      | A typed `MethodSignature` or `FieldSignature`        |
| `jni_sig_str!`  | A JNI descriptor as `&'static str`                   |
| `jni_sig_cstr!` | A modified UTF-8 JNI descriptor as `&'static CStr`   |
| `jni_sig_jstr!` | A JNI descriptor as `&'static JNIStr`                |

All signature variants accept the same signature syntax. Use `(parameters) -> return_type` for methods, a bare type for
fields, or a raw JNI descriptor such as `"(I)I"`. Parameter names are optional. Arrays support prefix (`[jint]`) and
suffix (`jint[]`) spelling.

String macros concatenate comma-separated literals, for example `jni_str!("java/lang/", "String")`. Their inputs must be
literals, not runtime variables; use `abos_jni::strings::JNIString` for dynamic JNI names.

### Bindings and native methods

| Macro                | Purpose                                                                                                              |
| -------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `bind_java_type!`    | Generate a typed Java wrapper, cached lookup metadata, constructors, methods, fields, and optional native interfaces |
| `native_method!`     | Produce a `NativeMethod` descriptor and optionally an exported JNI wrapper for a Rust implementation                 |
| `#[jni_mangle(...)]` | Set a JNI export name and the system ABI on a handwritten public function                                            |

`bind_java_type!` declarations can include `constructors`, `methods`, `fields`, and `native_methods` blocks. For
automatically generated declarations, use
[abos-jbindgen](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jbindgen/README.md).

## Scope and limitations

### Compile-time and runtime checks

The macros validate supported syntax and generate Rust code. Class loading, member lookup, and Java assignability checks
happen when the resulting bindings run. Compilation alone does not establish that the declarations match the Java API.
These macros do not compile Java sources or package Java classes.

### Java names and string encoding

Use dotted Java names in binding declarations and typed signatures, such as `java.lang.String`. Nested classes can use
`java.lang.Thread::State` or a quoted binary name such as `"java.lang.Thread$State"`. Default-package classes use a
leading dot, for example `".Example"`. Raw JNI descriptors retain slash-separated names, such as `"Ljava/lang/String;"`.

JNI strings are encoded as modified UTF-8, including embedded NULs and supplementary Unicode characters. A `CStr`
produced by these macros is therefore not necessarily ordinary UTF-8. Use the JNI string APIs for decoding rather than
assuming `CStr::to_str()` will accept every value.

### Native-call boundaries

Normal `native_method!` wrappers acquire an `Env`, catch unwinding Rust panics, and apply an error policy. The default
`ThrowRuntimeExAndDefault` policy preserves an already-pending Java exception; otherwise it translates Rust errors and
panics into a Java `RuntimeException` and returns a default JNI value. This does not recover from `panic = "abort"`.

Without export options, `native_method!` produces a descriptor for explicit registration. The `extern` qualifier with
`java_type` also requests a mangled JNI export. Raw wrappers and disabled panic catching require the caller to handle
the native boundary explicitly.

`jni_mangle` is a lower-level attribute, not a native-method wrapper. For example,
`#[jni_mangle("com.example.MathNative", "twice", "(I)I")]` sets an overload-specific export name. It requires a public
function, supplies or checks `extern "system"`, and does not validate the Rust parameter/return types against the
supplied Java descriptor. You must use FFI-safe types such as `EnvUnowned`, not `Env`, and provide exception and panic
handling. Prefer `native_method!` unless you need a handwritten entry point.

### Custom type mappings

`type_map` entries associate Rust wrapper paths with Java classes; `typealias` entries provide alternative names for an
existing mapping. The core Java types `Object`, `Class`, `String`, and `Throwable` in `java.lang` already have built-in
wrappers and cannot be remapped.

Custom primitives use `unsafe RustType => javaPrimitive`. They must match the JNI primitive's size and alignment and
provide the conversions used by the generated code. Calls into Java use `Into` conversions: arguments convert to the JNI
primitive, while returns convert back. Implement the required `From` conversions for your wrapper; it need not be
`Copy`. An unsafe mapping does not establish the validity or ownership of values such as native handles.

## Examples

### Describe signatures with a type map

Use aliases to share names within a signature declaration:

```rust,ignore
use abos_jni::{jni_sig, jni_sig_str};

fn main() {
    let method = jni_sig!(
        type_map = {
            User => com.example.User,
            typealias Text => JString,
        },
        (user: User, labels: Text[]) -> void
    );
    assert_eq!(method.sig().to_bytes(), b"(Lcom/example/User;[Ljava/lang/String;)V");
    assert_eq!(jni_sig_str!(jint[]), "[I");
    assert_eq!(jni_sig!("(I)I").sig().to_bytes(), b"(I)I");
}
```

### Define a typed Java wrapper

Declare the subset of a Java API your Rust code needs:

```rust,ignore
use abos_jni::sys::jint;
use abos_jni::{Env, bind_java_type};

bind_java_type! {
    pub JArrayList => "java.util.ArrayList",
    constructors {
        fn new(),
    },
    methods {
        fn size() -> jint,
    }
}

fn list_size(env: &mut Env<'_>) -> abos_jni::errors::Result<jint> {
    let list = JArrayList::new(env)?;
    list.size(env)
}
```

Call `list_size` from an attached thread. For classes requiring a specific loader, initialize the generated API with the
appropriate `LoaderContext` before calling its bindings.

### Export a native implementation

Match the Java declaration `public static native int twice(int value)` in `com.example.MathNative`:

```rust,ignore
use abos_jni::objects::JClass;
use abos_jni::sys::jint;
use abos_jni::{Env, NativeMethod, native_method};

const _: NativeMethod = native_method! {
    java_type = "com.example.MathNative",
    static extern fn twice(value: jint) -> jint,
};

fn twice(_env: &mut Env<'_>, _class: JClass<'_>, value: jint) -> abos_jni::errors::Result<jint> {
    Ok(value.wrapping_mul(2))
}
```

Build this as a `cdylib` and load it from Java. The macro generates the exported wrapper; the Rust implementation
returns `Result` rather than implementing the JNI ABI itself. See the main crate's quick start for the full Java loading
setup.

### Use a renamed runtime dependency

When Cargo names the runtime dependency differently, configure it explicitly:

```toml
[dependencies]
jni_runtime = { package = "abos-jni", version = "0.1.0" }
```

Pass `jni = <path>` as the first property in string, signature, and binding macro invocations:

```rust,ignore
use jni_runtime::{jni_sig, jni_str};

fn main() {
    let name = jni_str!(jni = ::jni_runtime, "java/lang/String");
    let signature = jni_sig!(jni = ::jni_runtime, (jint) -> JString);

    assert_eq!(name.to_bytes(), b"java/lang/String");
    assert_eq!(signature.sig().to_bytes(), b"(I)Ljava/lang/String;");
}
```

## Further examples

The runtime crate contains complete examples of
[signature syntax](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/jni_sig.rs),
[string literals](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/jni_str.rs),
[typed bindings](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/bind_java_type.rs),
and
[native methods](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/examples/native_method.rs).

From a checkout, run a runtime example with:

```sh
cargo run --package abos-jni --features invocation --example bind_java_type
```

These JVM examples compile Java fixtures and require an installed JDK. The string and signature examples can run without
a JVM, although their Cargo example targets also require the `invocation` feature to be enabled.

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-macros/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
