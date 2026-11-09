# abos-jni-sys-macros

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jni--sys--macros-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jni-sys-macros)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)

Versioned JNI and JVMTI function-table macros for abos-jni-sys.

## Installation

Most applications should use the tables already provided by
[abos-jni-sys](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys/README.md) or
[abos-jvmti-sys](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/README.md). Depend on
this proc-macro crate directly when defining or maintaining versioned function-table declarations:

```toml
[dependencies]
abos-jni-sys-macros = "0.1.0"
```

Expansion does not require a JDK or a running JVM. Generated views use `core` types and can be used in `no_std` crates.

## Quick Start

Describe the version in which each slot became available. This synthetic table demonstrates layout generation; it is not
an actual JNI table to pass to a JVM:

```rust
use abos_jni_sys_macros::jni_to_union;
use core::mem::{offset_of, size_of};

#[jni_to_union]
pub struct DemoTable {
    pub first: unsafe extern "system" fn(),
    #[jni_added("1.2")]
    pub added: unsafe extern "system" fn(),
    pub existing: unsafe extern "system" fn(),
    #[jni_added("9")]
    pub later: unsafe extern "system" fn(),
}

fn main() {
    let slot = size_of::<*const ()>();
    assert_eq!(size_of::<DemoTable_1_1>(), 3 * slot);
    assert_eq!(offset_of!(DemoTable_1_1, existing), 2 * slot);
    assert_eq!(size_of::<DemoTable_9>(), 4 * slot);
}
```

The macro replaces `DemoTable` with a union containing `v1_1`, `v1_2`, and `v9`. It also generates the corresponding
`DemoTable_1_1`, `DemoTable_1_2`, and `DemoTable_9` structs.

The baseline view keeps pointer-sized padding where `added` would be, so `existing` retains its slot offset. It omits
the trailing `later` slot. The unannotated `existing` field belongs to the baseline; it does not inherit the preceding
field's `1.2` annotation.

## Scope and limitations

### Choose the table representation

| Attribute           | Field annotation            | Result                                                                  |
| ------------------- | --------------------------- | ----------------------------------------------------------------------- |
| `#[jni_to_union]`   | `#[jni_added("version")]`   | Replaces the original struct with a union and version-specific structs  |
| `#[jvmti_versions]` | `#[jvmti_added("version")]` | Retains the original struct and adds version-specific structs; no union |

Unannotated fields default to JNI `1.1` or JVMTI `1.2`, respectively. Each annotation applies only to its own field.
Reserved slots must be marked explicitly with `"reserved"`; the field name alone does not identify them.

### Versions and generated names

Versions are string literals containing a numeric major version, an optional numeric minor version, or `"reserved"`.
Numeric components must fit in `u16`. Versions are sorted numerically; they are not checked against a list of published
JNI or JVMTI versions. Annotations model introduction of a slot, not later removal or signature changes.

| Annotation value | Generated type suffix | JNI union field |
| ---------------- | --------------------- | --------------- |
| `"1.2"`          | `_1_2`                | `v1_2`          |
| `"9"`            | `_9`                  | `v9`            |
| `"reserved"`     | `_reserved`           | `reserved`      |

Only versions represented by the fields generate views. For `jni_to_union`, a `"reserved"` annotation additionally
produces the full reserved view. `jvmti_versions` does not generate a reserved-only view.

### Layout and padding

Historical views retain field order and replace unavailable interior slots with `*mut core::ffi::c_void` padding. They
end at the last field available in that version rather than extending to the newest table's length.

JNI padding fields are private. JVMTI padding is public and named `reservedN`, where `N` is the one-based slot index.
Padding preserves offsets but is not a callable function or evidence of runtime support.

The macros are designed for pointer-sized function-table slots, not arbitrary versioned Rust data structures. They do
not validate field sizes, ABI signatures, slot order, or version annotations against platform headers. Incorrect source
declarations can therefore generate incorrect layouts.

### Struct requirements and attributes

Both macros take no arguments and require a nonempty struct with named fields. Enums, tuple structs, generics, and where
clauses are unsupported. Duplicate version annotations on a field and malformed version strings produce compile errors.

`jni_to_union` synthesizes `#[repr(C)]` structs deriving `Copy` and `Clone`, plus a `#[repr(C)]` union. It replaces the
original struct rather than forwarding its struct-level attributes. Field types must support the generated derives.

`jvmti_versions` retains the original struct's attributes; supply `#[repr(C)]` on that raw table yourself. Its generated
views use `#[repr(C)]` and derive `Clone`, `Copy`, `Debug`, and `Default`. Their field types must support those derives;
nullable `Option<unsafe extern "system" fn(...)>` entries and raw pointers are typical.

### Runtime safety

Generated views describe layouts; they do not negotiate a JNI/JVMTI version, load a JVM, or validate pointers. Confirm
runtime support before selecting a JNI union field or casting a JVMTI table pointer to a historical view.

Borrow a supported view rather than copying an entire table that may extend beyond an older runtime's allocation.
Reserved slots must never be called. A default-initialized JVMTI view can contain null pointers and `None` entries; it
is not an initialized runtime table. Every raw function call still requires its API's safety preconditions.

## Examples

### Keep a raw JVMTI table and add historical views

Use `jvmti_versions` when callers also need the complete original struct:

```rust
use abos_jni_sys_macros::jvmti_versions;
use core::mem::{offset_of, size_of};

#[jvmti_versions]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ToolTable {
    pub baseline: Option<unsafe extern "system" fn()>,
    #[jvmti_added("9")]
    pub modules: Option<unsafe extern "system" fn()>,
    pub existing: Option<unsafe extern "system" fn()>,
    #[jvmti_added("11")]
    pub sampling: Option<unsafe extern "system" fn()>,
}

fn main() {
    let slot = size_of::<*const ()>();
    let raw = ToolTable::default();
    assert!(raw.modules.is_none());
    assert_eq!(size_of::<ToolTable>(), 4 * slot);
    assert_eq!(size_of::<ToolTable_1_2>(), 3 * slot);
    assert_eq!(offset_of!(ToolTable_1_2, reserved2), slot);
    assert_eq!(offset_of!(ToolTable_1_2, existing), 2 * slot);
    assert_eq!(size_of::<ToolTable_11>(), 4 * slot);
}
```

`ToolTable` retains every declared field. `ToolTable_1_2` replaces `modules` with `reserved2` and excludes `sampling`;
`ToolTable_9` exposes `modules`, and `ToolTable_11` also exposes `sampling`. Like the quick-start table, this is a
layout example rather than a complete JVMTI interface.

## Tests and API reference

From a repository checkout, run the layout, version-parsing, and compile-fail tests:

```sh
cargo test --locked --package abos-jni-sys-macros
```

The [JNI declarations](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys/src/api/native.rs)
and
[JVMTI declarations](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/src/api/interface.rs)
show the attributes on complete function tables. The
[layout tests](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys-macros/tests/layout.rs)
cover interior padding, bounded views, and invalid input diagnostics. Generate the Rust reference locally with:

```sh
cargo doc --package abos-jni-sys-macros --no-deps --open
```

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys-macros/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
