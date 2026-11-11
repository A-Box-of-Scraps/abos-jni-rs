# abos-cafebabe

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--cafebabe-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-cafebabe)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Small Java class-file metadata reader for JNI binding generation.

## Installation

Add the dependency:

```toml
[dependencies]
abos-cafebabe = "0.1.0"
```

## Quick Start

Read a `.class` file and extract its metadata:

```rust,no_run
use abos_cafebabe::parse_class;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read("Example.class")?;
    let class = parse_class(&bytes)?;

    println!("{} (major version {})", class.this_class, class.major_version);
    for method in &class.methods {
        println!("{}{}", method.name, method.descriptor);
    }

    Ok(())
}
```

## Scope and limitations

### Supported input

The crate has no runtime dependencies and parses one complete class file from bytes. It does not read JAR archives or
Java source files.

Class-file major versions 45 through 71 are accepted. For major versions 56 and above, the minor version must be 0 or
65535 (preview).

### Extracted metadata

The result owns its metadata: versions, class and superclass names, direct interfaces, declared fields and methods, and
raw access flags. Methods include constructors (`<init>`) and class initializers (`<clinit>`). Descriptors remain JVM
strings rather than parsed types.

Attribute payloads are skipped, so bytecode, annotations, and generic signatures are not extracted.

### Validation and errors

This is a metadata reader, not a bytecode verifier. Unused constant-pool references and descriptor semantics are not
validated, so successful parsing does not establish that a JVM would accept the class.

Unsupported versions, unknown constant-pool tags, truncated structures, invalid metadata references or strings, and
trailing bytes produce a `ParseError` with a byte offset and a message.

### Text encoding

Modified UTF-8 is decoded only for extracted metadata. Unpaired surrogates in that metadata are rejected because Rust
strings cannot represent them. Unused Java string constants are not decoded and may contain unpaired surrogates.

## Examples

Names and descriptors retain their class-file representation:

| Metadata            | Example                                                          |
| ------------------- | ---------------------------------------------------------------- |
| Internal class name | `java/lang/String`                                               |
| Field descriptor    | `I` (`int`)                                                      |
| Method descriptor   | `(I)Ljava/lang/String;` (`int` parameter, `String` return value) |
| Array descriptor    | `[[Ljava/lang/String;` (`String[][]`)                            |

Find native methods using the named access flags:

```rust
use abos_cafebabe::{AccessFlags, ClassFile};

fn print_native_methods(class: &ClassFile) {
    for method in &class.methods {
        if method.access_flags.contains(AccessFlags::NATIVE) {
            println!("{}{}", method.name, method.descriptor);
        }
    }
}
```

Only selected flags have named constants. All bits are preserved and accessible through `AccessFlags.0`.

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-cafebabe/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
