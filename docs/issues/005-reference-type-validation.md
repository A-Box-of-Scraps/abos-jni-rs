# 005: Safe calls and field setters do not validate reference types

Priority: high.

Locations: `crates/abos-jni/src/api/env/calls.rs`, `crates/abos-jni/src/api/env/fields.rs`, and
`crates/abos-jni/src/api/signature/mod.rs`, `FieldSignature::type_is_compatible_with`.

The checks distinguish primitives from references but accept arbitrary objects for object or array signatures. JNI does
not guarantee a safe Java exception for incompatible arguments or field values.

Confirmed: `set_field` successfully stored a plain `Object` in a Java `String` field. Reading it back and checking
`instanceof String` returned false.

Fix: validate reference assignability against the actual parameter or field type. Check static setters, constructors,
and generated methods with unmapped object parameters too.
