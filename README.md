# jni-rs

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jni--rs-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs)
[![Downloads](https://img.shields.io/endpoint?url=https%3A%2F%2Fa-box-of-scraps.github.io%2Fabos-jni-rs%2Fdownloads.json&style=for-the-badge&labelColor=555555&color=00599C)](.)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Rust bindings and utilities for the Java Native Interface.

## Crates

### [abos-cafebabe](crates/abos-cafebabe)

Small Java class-file metadata reader for JNI binding generation.

### [abos-javac](crates/abos-javac)

A build-time dependency for Cargo build scripts and unit tests to assist in invoking the native Java compiler to compile
Java source files into class files.

### [abos-jbindgen](crates/abos-jbindgen)

Generate Rust JNI bindings from Java source, bytecode, and Android SDKs.

### [abos-jbindgen-annotations](crates/abos-jbindgen-annotations)

Embedded Java annotations for abos-jbindgen.

### [abos-jni](crates/abos-jni)

Rust bindings and utilities for the Java Native Interface.

### [abos-jni-macros](crates/abos-jni-macros)

Procedural macros for abos-jni.

### [abos-jni-sys](crates/abos-jni-sys)

Raw Rust definitions corresponding to jni.h.

### [abos-jni-sys-macros](crates/abos-jni-sys-macros)

Versioned JNI and JVMTI function-table macros for abos-jni-sys.

### [abos-jvmti](crates/abos-jvmti)

Environment and agent utilities for the Java Virtual Machine Tool Interface.

### [abos-jvmti-sys](crates/abos-jvmti-sys)

Raw Rust definitions corresponding to jvmti.h.

## Supported platforms

We currently support the following platforms:

- Linux AMD64
- Linux ARM64
- Windows AMD64
- Windows ARM64
- macOS AMD64
- macOS ARM64

## Contributing

Contributions are welcome!

- Read the contributor guide in [CONTRIBUTING.md](CONTRIBUTING.md).
- Report bugs or request features in [Issues](https://github.com/A-Box-of-Scraps/abos-jni-rs/issues/)
- Open changes through [Pull Requests](https://github.com/A-Box-of-Scraps/abos-jni-rs/pulls/)

> All pull requests must be tested before being submitted.

## License

Licensed under the [MIT License](LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES): for attribution and additional notices.
