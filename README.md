# jni-rs

Rust bindings and utilities for the Java Native Interface.

## Crates

### [abos-cafebabe](crates/abos-cafebabe)

Small Java class-file metadata reader for JNI binding generation.

### [abos-javac](crates/abos-javac)

A build-time dependency for Cargo build scripts and unit tests to assist in invoking the native Java compiler to compile Java source files into class files.

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

## License

Licensed under the [MIT License](LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES): for attribution and additional notices.
