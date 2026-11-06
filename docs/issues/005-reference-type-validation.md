# 005: Safe calls and field setters do not validate reference types

Status: fixed.

Priority: high.

Locations: `crates/abos-jni/src/api/env/calls.rs`, `crates/abos-jni/src/api/env/fields.rs`, and
`crates/abos-jni/src/api/signature/mod.rs`, `FieldSignature::type_is_compatible_with`.

The checks distinguish primitives from references but accept arbitrary objects for object or array signatures. JNI does
not guarantee a safe Java exception for incompatible arguments or field values.

Confirmed: `set_field` successfully stored a plain `Object` in a Java `String` field. Reading it back and checking
`instanceof String` returned false.

Resolution: safe calls, constructors, and field setters validate non-null references against the resolved member's
reflected parameter or field classes, preserving class-loader identity. Generated bindings use the same checks,
including unmapped object and array parameters, and evaluate `AsRef` only once before validation and invocation. Null
references remain valid, and incompatible references return `Error::WrongObjectType` before calling or writing.
Regression tests cover object and array mismatches, interfaces, nulls, unchanged fields, nonvirtual calls, generated
bindings, inherited members, and same-named classes loaded by distinct class loaders under `-Xcheck:jni`.
