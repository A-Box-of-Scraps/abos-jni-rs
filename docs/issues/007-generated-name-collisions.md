# 007: Name collision handling ignores macro-generated methods

Status: fixed.

Priority: medium.

Locations: `crates/abos-jbindgen/src/internal/generator/mod.rs`, the member-name registry, and
`crates/abos-jni-macros/src/internal/bind_java_type/references.rs`, `generate_base_methods`.

The generator reserves names used by Java members but not names already emitted by the binding macro.

Confirmed: legal Java methods `fromRaw()` and `intoRaw()` generated Rust methods named `from_raw` and `into_raw`,
colliding with the wrapper's built-in methods. Compilation failed with duplicate-definition errors.

Resolution: member-name allocation reserves the wrapper's inherent reference methods, reference helpers used by
generated calls, and generated superclass accessors. Conflicting Java members and explicit overrides gain trailing
underscores while preserving their JNI lookup names. Module-level validation also detects conflicts with API types,
native interfaces, package modules, and `jni_init`, honoring disabled or skipped helpers. As with existing type-name
collisions, these conflicts produce generation errors rather than invalid Rust.

Compile-and-run regressions cover source and bytecode bindings, fields, constructors, instance and static methods,
overloads, native interfaces, and continued access to the wrapper's reference API. Additional tests cover module-level
conflicts and configurations that disable the conflicting helpers.
