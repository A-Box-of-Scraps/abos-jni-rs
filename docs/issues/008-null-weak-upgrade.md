# 008: Null weak references upgrade to Some(null)

Status: fixed.

Priority: medium.

Location: `crates/abos-jni/src/api/refs/weak.rs`, `Weak::upgrade_local` and `Weak::upgrade_global`.

Both methods wrap successful results in `Some` without checking whether the resulting reference is null. The underlying
reference-creation methods return a successful null reference for a null input.

Confirmed: both upgrade methods on `Weak::null()` returned `Ok(Some(null))`, although null weak references are
documented to behave as collected references.

Resolution: both upgrade methods return `None` when reference creation succeeds with a null reference, while retaining
existing collected-reference and error handling. Regression tests cover local and global upgrades of null and default
object/string weak references, plus pending-exception propagation. Existing tests cover live and collected referents,
typed upgrades, and cross-thread use under `-Xcheck:jni`.
