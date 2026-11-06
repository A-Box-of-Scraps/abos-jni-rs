# 002: Weak references expose operations requiring a live object

Status: fixed.

Priority: high.

Location: `crates/abos-jni/src/api/refs/weak.rs`, the `AsRef` and `Deref` implementations.

`Weak<T>` exposes the underlying object wrapper, allowing safe callers to bypass upgrading. The referent can be
collected at any time; checking collection status before using the reference does not prevent that race.

Confirmed by collecting the referent and calling `env.get_object_class(&weak)`: the JVM aborted under `-Xcheck:jni` with
a bad-reference error.

Resolution: `Weak` no longer implements `AsRef` or `Deref`. Reference creation and identity comparisons use `Reference`
directly, while object-access APIs still require a strong typed view. Compile-fail tests reject direct object access and
casts; runtime tests cover typed upgrades, collection, identity, and cross-thread use.
