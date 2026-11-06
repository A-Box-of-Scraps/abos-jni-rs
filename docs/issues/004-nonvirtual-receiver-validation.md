# 004: Nonvirtual calls do not validate the receiver's class

Priority: high.

Location: `crates/abos-jni/src/api/env/calls.rs`, `Env::call_nonvirtual_method`.

The safe method resolves the supplied class and method without checking that the receiver belongs to that class
hierarchy. It then passes the incompatible receiver and method ID to JNI.

Confirmed by calling `String.length()` nonvirtually on an ordinary `Object`: the JVM aborted under `-Xcheck:jni` with
`Wrong object class or methodID passed to JNI call`.

Fix: validate receiver assignability before making the unchecked call.
