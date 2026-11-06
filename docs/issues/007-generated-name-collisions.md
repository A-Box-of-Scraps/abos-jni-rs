# 007: Name collision handling ignores macro-generated methods

Priority: medium.

Locations: `crates/abos-jbindgen/src/internal/generator/mod.rs`, the member-name registry, and
`crates/abos-jni-macros/src/internal/bind_java_type/references.rs`, `generate_base_methods`.

The generator reserves names used by Java members but not names already emitted by the binding macro.

Confirmed: legal Java methods `fromRaw()` and `intoRaw()` generated Rust methods named `from_raw` and `into_raw`,
colliding with the wrapper's built-in methods. Compilation failed with duplicate-definition errors.

Fix: seed collision handling with generated method names. Also account for generated API types and helper
symbols, and compile regression fixtures rather than checking generated text alone.
