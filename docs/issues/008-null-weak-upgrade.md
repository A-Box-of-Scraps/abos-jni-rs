# 008: Null weak references upgrade to Some(null)

Priority: medium.

Location: `crates/abos-jni/src/api/refs/weak.rs`, `Weak::upgrade_local` and `Weak::upgrade_global`.

Both methods wrap successful results in `Some` without checking whether the resulting reference is null. The underlying
reference-creation methods return a successful null reference for a null input.

Confirmed: both upgrade methods on `Weak::null()` returned `Ok(Some(null))`, although null weak references are
documented to behave as collected references.

Fix: return `None` for null upgrade results. Add regression tests for both local and global upgrades.
