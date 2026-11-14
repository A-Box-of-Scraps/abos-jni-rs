# abos-javac

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--javac-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-javac)
[![crates.io](https://img.shields.io/crates/v/abos-javac?style=for-the-badge&labelColor=555555&color=00599C)](https://crates.io/crates/abos-javac)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Compile Java sources with `javac` from Cargo build scripts, tests, or Rust applications.

## Installation

Install a JDK containing `javac`; the crate does not install Java for you.

For a Cargo build script, add:

```toml
[build-dependencies]
abos-javac = "0.1.0"
```

Use `[dev-dependencies]` for tests or `[dependencies]` for Rust applications instead.

## Quick Start

Create `java/Example.java`:

```java
public class Example {}
```

Then add a `build.rs` at your Cargo package root:

```rust,no_run
fn main() -> abos_javac::Result<()> {
    abos_javac::Build::new()
        .source_dir("java")
        .encoding("UTF-8")
        .try_compile()?;

    Ok(())
}
```

The build script recursively finds `.java` files under `java` and compiles them. `try_compile()` returns a
`Vec<PathBuf>` containing the generated class-file paths.

## Scope and limitations

### Compiler discovery

The crate looks for `javac` in `JAVA_HOME/bin` first, then falls back to `PATH` if no compiler file is found there. Java
language features and available target releases depend on the selected compiler.

Use `.release("17")`, for example, to pass `--release 17`. This option requires JDK 9 or newer and overrides
`.source_version(...)` and `.target_version(...)`.

### Sources and outputs

Select individual files with `.file(...)` or `.files(...)`, or recursively scan directories with `.source_dir(...)`.
Directory symlinks are not followed. Classpath entries can be directories or JAR files, added with `.classpath(...)`.

By default, class files go to `$OUT_DIR/javac-build/classes` and generated Java sources go to
`$OUT_DIR/javac-build/generated-sources`. `.output_subdir(...)` appends a path to the class output directory.

Outside a Cargo build script, set `.output_dir(...)` explicitly rather than relying on `OUT_DIR`. With an explicit class
output directory, generated sources default to that resolved directory too; use `.output_src_dir(...)` to separate them.

Returned paths include only class files reported by the current compiler invocation, including nested classes. Existing
files in the output directory are not automatically removed or included in the result.

### Cargo integration

Compilation emits Cargo rebuild directives for selected sources, scanned source directories, classpath entries,
`JAVA_HOME`, and `PATH`. This tracking is enabled by default.

Use `.cargo_metadata(false)` in applications or tests to disable Cargo directives. In a build script that provides its
own input-change tracking, `.emit_rerun_if_changed(false)` disables file and directory directives while keeping
`JAVA_HOME` and `PATH` tracking enabled.

### Errors

Use `try_compile()` to handle failures as `Result` values. `compile()` is the panicking alternative. Compiler failures
include captured stdout and stderr in `Error::CompilationFailed`.

An empty source selection is an error. A successful compiler invocation that reports no class files is also an error
(`Error::NoClassFilesGenerated`).

## Examples

### Configure a classpath and target release

Compile sources against a dependency JAR, targeting a release supported by the selected JDK:

```rust,no_run
fn main() -> abos_javac::Result<()> {
    abos_javac::Build::new()
        .source_dir("java")
        .classpath("lib/dependency.jar")
        .release("17")
        .encoding("UTF-8")
        .try_compile()?;

    Ok(())
}
```

### Compile outside a build script

Set an output directory and disable Cargo directives when compiling from an application or test:

```rust,no_run
fn main() -> abos_javac::Result<()> {
    let classes = abos_javac::Build::new()
        .file("java/Example.java")
        .output_dir("target/java-classes")
        .cargo_metadata(false)
        .try_compile()?;

    for class in classes {
        println!("{}", class.display());
    }

    Ok(())
}
```

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-javac/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
