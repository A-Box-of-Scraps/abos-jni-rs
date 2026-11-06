# 006: Default-package type references generate invalid bindings

Priority: medium.

Locations: `crates/abos-jbindgen/src/api/type_map.rs`, `TypeMap::from_classes`, and
`crates/abos-jbindgen/src/internal/generator/names.rs`, `resolve_type_with_deps`.

Default-package classes are registered as `.DefaultType`, but descriptor-derived references use `DefaultType`. The
lookup misses, and the fallback emits a class-name spelling the binding macro rejects.

Confirmed: `public DefaultType echo(DefaultType other)` generated `fn echo(arg0: "DefaultType") -> "DefaultType",` and
the generated Rust failed to compile.

Fix: normalize Java names consistently across descriptors, type-map keys, and macro output. Add a compile test for
default-package classes referencing themselves and each other.
