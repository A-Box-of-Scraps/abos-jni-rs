# 003: Monitor guards can outlive their object reference

Priority: high.

Locations: `crates/abos-jni/src/api/env/native.rs`, `Env::lock_obj`, and
`crates/abos-jni/src/api/env/mod.rs`, `MonitorGuard`.

The guard stores a raw object reference, but its lifetime is tied only to the environment. The original reference
can be deleted before the guard calls `MonitorExit`.

Confirmed using only safe calls: lock a global reference, drop the global reference, then drop the guard. The JVM
aborted under `-Xcheck:jni` with a bad-reference error.

Fix: borrow the reference owner for the guard's lifetime, or retain an independent strong reference.
