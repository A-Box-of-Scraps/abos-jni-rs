# abos-jbindgen-annotations

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jbindgen--annotations-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jbindgen-annotations)
[![crates.io](https://img.shields.io/crates/v/abos-jbindgen-annotations?style=for-the-badge&labelColor=555555&color=00599C)](https://crates.io/crates/abos-jbindgen-annotations)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Embedded Java annotations for abos-jbindgen.

## Installation

Add the crate to the Rust application that exports your annotation sources:

```toml
[dependencies]
abos-jbindgen-annotations = "0.1.0"
```

Use `[build-dependencies]` when exporting from `build.rs`. Exporting sources requires no runtime dependencies, JDK, or
JVM. For compilation and binding generation, see
[abos-jbindgen](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jbindgen/README.md).

## Quick Start

Export the Java sources into a source root:

```rust,no_run
fn main() -> std::io::Result<()> {
    let files = abos_jbindgen_annotations::write_sources("java")?;
    for file in files {
        println!("{}", file.display());
    }
    Ok(())
}
```

This creates `java/io/github/jni_rs/jbindgen/` containing `RustName.java`, `RustSkip.java`, `RustPrimitive.java`, and
`package-info.java`. With the `abos-jbindgen` CLI installed, `abos-jbindgen annotations --output java` exports the same
files without a Rust application.

Create `java/com/example/Calculator.java`:

```java
package com.example;

import io.github.jni_rs.jbindgen.RustName;
import io.github.jni_rs.jbindgen.RustSkip;

@RustName("CalculatorBindings")
public class Calculator {
    @RustName("sum")
    public static int add(int left, int right) {
        return left + right;
    }

    @RustSkip
    public static void debug() {}
}
```

Generate from the source tree, including the exported annotation declarations:

```sh
abos-jbindgen java java --pattern com.example.Calculator --root crate::bindings --output bindings.rs
```

The generated class wrapper is named `CalculatorBindings`, `add` is exposed as `sum`, and `debug` is omitted. The
pattern selects the application class rather than generating wrappers for the annotation declarations. See the generator
README for including and initializing the resulting bindings.

## Scope and limitations

### Annotation behavior

All annotations belong to the Java package `io.github.jni_rs.jbindgen`.

| Annotation                   | Applies to                             | Effect during source-based generation                             |
| ---------------------------- | -------------------------------------- | ----------------------------------------------------------------- |
| `@RustName("name")`          | Classes, constructors, methods, fields | Overrides the generated Rust name                                 |
| `@RustSkip`                  | Classes, constructors, methods, fields | Omits the annotated declaration from bindings                     |
| `@RustPrimitive("RustType")` | Parameters                             | Uses a configured custom Rust type for a Java primitive parameter |

Names supplied through `@RustName` are normalized to Rust identifiers and may gain trailing underscores to avoid member
collisions. Builder name overrides take precedence over the corresponding annotations. Java names and signatures remain
unchanged for JNI lookup.

These annotations customize generated bindings, not Java behavior. Skipping a declaration does not remove it from the
Java class or restrict access to it.

### Source analysis and bytecode

The annotations use `RetentionPolicy.RUNTIME`, so they can remain in compiled class files. However, `abos-jbindgen` does
not read them from class-file or bytecode JAR inputs. Use Java-source or source-JAR input to apply them during
generation. Annotation declarations must be available as source inputs or through the analysis classpath.

For bytecode workflows, use the builder's `.name_override(...)` and `.skip_signature(...)`, or the CLI's `--name` and
`--skip`, to rename or omit members without annotations.

### Export behavior

`write_sources(root)` creates the package directory beneath `root`, replaces existing files with the exported names, and
returns their paths. Other files are left untouched. Filesystem failures are returned as `std::io::Error`.

`PACKAGE` exposes the Java package name, and `ANNOTATION_FILES` exposes each filename and its UTF-8 source text for
custom export workflows. This crate supplies source files only: it does not invoke a compiler or generate Rust bindings.

## Examples

### Use a custom primitive parameter type

Create `java/com/example/Handles.java` alongside the exported annotation sources:

```java
package com.example;

import io.github.jni_rs.jbindgen.RustPrimitive;

public class Handles {
    public static long value(@RustPrimitive("crate::Handle") long handle) {
        return handle;
    }
}
```

The annotation requires an explicit unsafe primitive mapping with exactly the same Rust type spelling and matching Java
primitive:

```sh
abos-jbindgen java java --pattern com.example.Handles --type-map 'unsafe crate::Handle=>long' --root crate::bindings --output handles.rs
```

The builder equivalent is `.type_mapping("unsafe crate::Handle", "long")`. Missing mappings, mismatched primitives, and
annotations on array or reference parameters cause generation errors. The annotation applies to the parameter, not the
method's return type; the Java signature still uses `long`.

In the consuming Rust crate, define the mapped type and its conversion to the JNI primitive:

```rust,ignore
#[repr(transparent)]
pub struct Handle(pub abos_jni::sys::jlong);

impl From<Handle> for abos_jni::sys::jlong {
    fn from(handle: Handle) -> Self {
        handle.0
    }
}
```

Custom primitive types must have the same size and alignment as their JNI primitive and provide the conversions used by
the generated bindings. The mapping is unsafe because those representation and conversion requirements are the caller's
responsibility; the annotation does not establish them.

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jbindgen-annotations/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
