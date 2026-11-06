# 001: Failed Rust field take frees a still-owned value

Status: fixed.

Priority: high.

Location: `crates/abos-jni/src/api/env/rust_fields.rs`, `Env::take_rust_field`.

The function reconstructs a `Box<Mutex<T>>` before calling `try_lock`. If locking fails, returning the error drops that
box even though an existing `MutexGuard` still borrows the allocation. The Java field retains the dangling pointer,
allowing use-after-free or double-free.

Confirmed by holding a guard and taking the field: the call returned `Err(TryLock)`, the value's destructor ran, and the
Java handle remained nonzero.

Resolution: the pointer stays borrowed until locking and clearing the Java field succeed, then ownership transfers with
`Box::from_raw`. Regression tests cover successful transfer, locked and poisoned mutexes, preserved handles, and
exactly-once destruction.
