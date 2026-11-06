# 006: Default-package type references generate invalid bindings

Status: fixed.

Priority: medium.

Locations: `crates/abos-jbindgen/src/api/type_map.rs`, `TypeMap::from_classes`, and
`crates/abos-jbindgen/src/internal/generator/names.rs`, `resolve_type_with_deps`.

Default-package classes are registered as `.DefaultType`, but descriptor-derived references use `DefaultType`. The
lookup misses, and the fallback emits a class-name spelling the binding macro rejects.

Confirmed: `public DefaultType echo(DefaultType other)` generated `fn echo(arg0: "DefaultType") -> "DefaultType",` and
the generated Rust failed to compile.

Resolution: type maps use canonical Java binary names internally and accept both `DefaultType` and `.DefaultType`
inputs. Generated class names, type-map entries, exported mappings, and unmapped reference types use the macro's
`.DefaultType` spelling. Compile-and-run regressions cover source and bytecode input, self and cross-class references,
constructors, fields, arrays, nested classes, inheritance, explicit mappings, and unmapped fallbacks.
