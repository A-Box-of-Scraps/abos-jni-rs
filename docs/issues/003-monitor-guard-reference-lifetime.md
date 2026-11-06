# 003: Monitor guards can outlive their object reference

Status: fixed.

Priority: high.

Locations: `crates/abos-jni/src/api/env/native.rs`, `Env::lock_obj`, and `crates/abos-jni/src/api/env/mod.rs`,
`MonitorGuard`.

The guard stores a raw object reference, but its lifetime is tied only to the environment. The original reference can be
deleted before the guard calls `MonitorExit`.

Confirmed using only safe calls: lock a global reference, drop the global reference, then drop the guard. The JVM
aborted under `-Xcheck:jni` with a bad-reference error.

Resolution: `lock_obj` borrows its object reference, and the guard cannot outlive that borrow or the environment's
attachment lifetime. No extra JNI reference is allocated. Rust field operations retain their reference owners until
unlocking. Compile-fail tests cover owner deletion, frame escape, and attachment escape; runtime tests cover reentrancy,
unwinding, pending exceptions, and owned global references.
