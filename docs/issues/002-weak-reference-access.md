# 002: Weak references expose operations requiring a live object

Priority: high.

Location: `crates/abos-jni/src/api/refs/weak.rs`, the `AsRef` and `Deref` implementations.

`Weak<T>` exposes the underlying object wrapper, allowing safe callers to bypass upgrading. The referent can be
collected at any time; checking collection status before using the reference does not prevent that race.

Confirmed by collecting the referent and calling `env.get_object_class(&weak)`: the JVM aborted under
`-Xcheck:jni` with a bad-reference error.

Fix: remove strong-object access through `Weak`. Require an upgrade before ordinary object operations and retain
explicit weak-safe operations.
