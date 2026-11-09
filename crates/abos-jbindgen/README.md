# abos-jbindgen

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jbindgen-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jbindgen)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)

Generate Rust JNI bindings from Java source, bytecode, and Android SDKs.

## Installation

Building the crate requires a JDK capable of compiling with `javac --release 11`. Set `JAVA_HOME` to your JDK
installation; `abos-javac` also searches `PATH` for `javac`.

Java-source and Android generation use an in-process JVM with the JDK compiler API. Class-file and bytecode JAR
generation do not start a JVM.

For the build-script workflow below, add these dependencies to your Cargo package:

```toml
[dependencies]
abos-jni = { version = "0.1.0", features = ["invocation"] }

[build-dependencies]
abos-javac = "0.1.0"
abos-jbindgen = "0.1.0"
```

The `invocation` feature is needed here because the Rust application starts its own JVM. Use `abos-jbindgen` under
`[dependencies]` instead when calling the generator from a Rust application.

To install the CLI from a checkout, run from the repository root:

```sh
cargo install --path crates/abos-jbindgen
```

## Quick Start

### Compile Java and generate bindings

Create `java/com/example/Calculator.java` in your Cargo package:

```java
package com.example;

public class Calculator {
    public static int add(int left, int right) {
        return left + right;
    }
}
```

Add a `build.rs` that compiles the Java class and generates Rust bindings from its bytecode:

```rust
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    let classes = output.join("java-classes");

    abos_javac::Build::new()
        .file("java/com/example/Calculator.java")
        .output_dir(&classes)
        .encoding("UTF-8")
        .try_compile()?;

    abos_jbindgen::Builder::new()
        .input_class(classes.join("com/example/Calculator.class"))
        .root_path("crate::bindings")
        .generate()?
        .write_to_file(output.join("bindings.rs"))?;

    Ok(())
}
```

`abos-javac` emits Cargo rebuild directives for the Java input and compiler environment. `abos-jbindgen` does not emit
rebuild directives itself: when generating directly from existing inputs, track them in your build script with
`cargo:rerun-if-changed`, along with any environment settings that affect generation.

### Include and call the bindings

In `src/main.rs`, include the generated file inside the module named by `.root_path(...)`:

```rust
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let classes = concat!(env!("OUT_DIR"), "/java-classes");
    let args = abos_jni::InitArgsBuilder::new()
        .version(abos_jni::JNIVersion::V1_8)
        .option(format!("-Djava.class.path={classes}"))
        .build()?;
    let vm = abos_jni::JavaVM::new(args)?;

    vm.attach_current_thread(|env| -> abos_jni::errors::Result<()> {
        bindings::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
        assert_eq!(bindings::com::example::Calculator::add(env, 20, 22)?, 42);
        Ok(())
    })?;

    Ok(())
}
```

Run the application with `cargo run`. Generating bindings does not compile or package Java classes for runtime use;
`abos-javac` performs that separate compilation step in this example. The JVM classpath points to build output here. For
distribution, package the classes or JARs separately and configure the runtime classpath accordingly.

## Command-line usage

Generate from a class file, a bytecode JAR, Java sources, or an installed Android platform:

```sh
abos-jbindgen classfile classes/com/example/Calculator.class --root crate::bindings --output bindings.rs
abos-jbindgen classfile library.jar --pattern 'com.example.*' --root crate::bindings --output bindings.rs
abos-jbindgen java java --classpath lib/dependency.jar --root crate::bindings --output bindings.rs
abos-jbindgen android --api-level 35 --pattern 'android.os.*' --root crate::android --output android.rs
```

Without an output option, bindings are written to stdout. `--output` writes one Rust source file; `--output-dir` writes
a module tree rooted at `mod.rs`. These output options are mutually exclusive. Set `--root` to the Rust module that will
contain the generated code.

Use `abos-jbindgen --help` or a subcommand's `--help` for the full option reference. The `annotations` subcommand
exports Java binding-annotation sources, for example `abos-jbindgen annotations --output java`.

## Scope and limitations

### Inputs and class selection

The builder accepts a single `.class` file or its bytes, a bytecode JAR, Java source files/directories/source JARs, or
an Android API level. Source analysis reads declarations; it does not produce runtime class files. Selected local
classes are not supported by source analysis because their captured constructor parameters are not exposed; use bytecode
input for those classes.

Set the input before calling `.patterns(...)`. Patterns apply to bulk inputs, not a single class file. Matching is
case-sensitive, and repeated or comma-separated patterns form a union. An empty pattern list selects all eligible
classes; a nonempty list matching nothing succeeds with empty bindings.

| Pattern                  | Selection                                                               |
| ------------------------ | ----------------------------------------------------------------------- |
| `com.example.Calculator` | The class and all its nested descendants                                |
| `com.example.*`          | All matching classes, including subpackages                             |
| `com.example.Outer$*`    | Nested classes, but not `Outer` itself                                  |
| `*Inner`                 | Class names ending in `Inner`, without implicitly selecting descendants |

`*` matches any characters, including package and nesting separators. Binary nested names (`Outer$Inner`) and dotted
aliases (`Outer.Inner`) are accepted; `/` can replace package separators. Quote CLI patterns to prevent shell expansion.

### Generated code and runtime integration

Generated code uses `abos_jni::bind_java_type!` and requires `abos-jni` in the consuming crate. Java packages become
Rust modules. `.root_path(...)` must match where the bindings are included; its default is `crate`.

Initialization helpers and native-method interfaces are enabled by default. Call the generated `jni_init` with the
appropriate `LoaderContext` to initialize the bindings with the intended class loader. Native interfaces describe Rust
implementations of Java native methods; generation alone does not implement those methods. Android generation disables
native-interface generation because the platform implements its native methods.

### Names and type mappings

Rust identifiers use ASCII spelling: keywords gain a trailing underscore, punctuation becomes underscores, and non-ASCII
characters become `_u<hex>_`. Original Java names remain intact for JNI lookup. Overloads receive distinct names; member
collisions gain trailing underscores, while conflicting module-level declarations fail generation.

`.type_mapping(rust_path, java_name)` connects references to existing Rust wrappers. The Java core classes `Object`,
`Class`, `String`, and `Throwable` in `java.lang` use built-in `abos-jni` wrappers and cannot be remapped. Bulk
generation omits their declarations; requesting one as a single class or exact pattern is an error.

### JAR behavior

Bytecode JAR generation processes base class entries in archive-name order. It excludes `module-info.class`,
`package-info.class`, and everything under `META-INF/`, including multi-release variants. Version-only classes and
members are not generated, and there is no target-release selection.

The JVM still selects its runtime variant, which must retain the members used by the generated base bindings.

### Android behavior and errors

Android generation finds the SDK through `ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT` when unset. The selected
`platforms/android-<level>` directory must contain both `android.jar` and `android-stubs-src.jar`. Nothing is
downloaded.

The generated API combines platform bytecode with source stubs analyzed against that platform, rather than the host
JDK's core classes. Optional `hiddenapi-flags.csv` filtering further restricts the selected API surface; it does not
bypass Android's runtime access restrictions.

Generation returns errors for invalid inputs, parsing failures, and conflicting generated declarations. Android
selected-class failures retain their context and underlying cause; generation does not silently return partial bindings.
Use class patterns or member skips to explicitly restrict the API surface.

## Examples

### Filter a JAR

Select a package and generate bindings under `crate::bindings`:

```rust
fn main() -> abos_jbindgen::Result<()> {
    abos_jbindgen::Builder::new()
        .input_jar("library.jar")
        .patterns(["com.example.*"])
        .root_path("crate::bindings")
        .generate()?
        .write_to_file("bindings.rs")
}
```

### Generate Android bindings

With the required platform files installed, select an API level and package:

```rust
fn main() -> abos_jbindgen::Result<()> {
    abos_jbindgen::Builder::new()
        .input_android_sdk(35, vec!["android.os.*".into()])
        .root_path("crate::android")
        .generate()?
        .write_to_file("android.rs")
}
```

To additionally filter hidden APIs, call `.hiddenapi_flags("hiddenapi-flags.csv")` after selecting the Android input.
`.allow_unsupported(true)` and `.max_target(...)` control that filter's eligibility rules, not error handling.

### Rename or skip a member

Use a DEX signature to identify a specific overload. This example renames `add(int, int)` to `sum` and omits the default
constructor:

```rust
fn main() -> abos_jbindgen::Result<()> {
    abos_jbindgen::Builder::new()
        .input_class("classes/com/example/Calculator.class")
        .root_path("crate::bindings")
        .name_override("Lcom/example/Calculator;->add(II)I", "sum")
        .skip_signature("Lcom/example/Calculator;-><init>()V")
        .generate()?
        .write_to_file("bindings.rs")
}
```

Name overrides take precedence over `@RustName` annotations. Field signatures use the form
`Lcom/example/Calculator;->value:I`.

### Export a module tree and type map

Write a directory tree rooted at `generated/mod.rs` and a reusable type-mapping file:

```rust
fn main() -> abos_jbindgen::Result<()> {
    let bindings = abos_jbindgen::Builder::new()
        .input_jar("library.jar")
        .patterns(["com.example.*"])
        .root_path("crate::bindings")
        .generate()?;

    bindings.write_to_files("generated")?;
    bindings.write_type_map("bindings.types")?;
    Ok(())
}
```

Include the tree's `mod.rs` under the configured root module. The CLI imports exported mapping files with `--type-map`;
the library accepts individual `(Rust type path, Java binary name)` mappings through `.type_mapping(...)`. Use
`write_pub_type_map(...)` when exported mappings need a different public Rust root path.

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
