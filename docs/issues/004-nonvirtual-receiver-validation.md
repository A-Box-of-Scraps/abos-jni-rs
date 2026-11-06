# 004: Nonvirtual calls do not validate the receiver's class

Status: fixed.

Priority: high.

Location: `crates/abos-jni/src/api/env/calls.rs`, `Env::call_nonvirtual_method`.

The safe method resolves the supplied class and method without checking that the receiver belongs to that class
hierarchy. It then passes the incompatible receiver and method ID to JNI.

Confirmed by calling `String.length()` nonvirtually on an ordinary `Object`: the JVM aborted under `-Xcheck:jni` with
`Wrong object class or methodID passed to JNI call`.

Resolution: `call_nonvirtual_method` checks receiver assignability against the resolved class before making the
unchecked call, returning `Error::WrongObjectType` for incompatible receivers. Regression tests cover superclass and
unrelated-class mismatches, named and resolved class descriptors, and null receivers. Existing tests verify exact-class
and superclass nonvirtual dispatch. All run under `-Xcheck:jni`.
