//! Utilities for JVMTI 1.2-or-newer agents (Java 8+), interoperable with [`abos_jni`].
//!
//! Default constructors request the 1.2 baseline once; explicit-version constructors
//! can require newer interfaces. [`Env::version`] retains the actual version without
//! a native call. Newer callbacks, events, and capabilities are version-checked,
//! but availability and execution-context requirements still come from the VM.
//!
//! [`Env`] is a borrowed, cross-thread environment handle, not a JNI attachment
//! guard. JVM calls remain unsafe: callers must satisfy their JVMTI phase,
//! callback-context, and thread-attachment requirements. Environments are not
//! automatically disposed, since disposal invalidates callbacks and other handles.
//!
//! # Agent operations
//!
//! [`Env`] wraps class retransformation/redefinition, owned method bytecodes,
//! breakpoints and per-thread stepping, typed local-variable reads/writes, stack
//! traces, thread suspension/resumption, tags, synchronous heap traversal, event
//! filtering, capability relinquishment, and agent-owned environment storage.
//! These are available at the JVMTI 1.2 baseline. Bulk virtual-thread suspension
//! and resumption require JVMTI 21 and are checked before borrowing newer slots.
//!
//! [`ClassDefinition`] borrows complete replacement class files. Stack traces
//! use Rust-owned vectors; bytecodes use [`JvmtiArray`]. Returned object locals
//! remain independently managed JNI references. Heap callbacks borrow their
//! arguments and must obey restricted callback rules, without unwinding.
//!
//! [`RawMonitor`] owns a recursive native monitor; [`RawMonitorGuard`] owns one
//! entry and cannot leave the acquiring thread. Explicit `destroy` and `unlock`
//! report failures; drop attempts cleanup once. Neither raw-monitor locking nor
//! environment storage automatically establishes safe Rust data synchronization.
//!
//! Every operation documents its capability, phase, callback, attachment, and
//! ownership contract. Native availability errors are preserved rather than
//! prechecked with additional calls that could race capability/phase changes.
//!
//! # Returned-data ownership
//!
//! [`JvmtiArray`] adopts initialized flat buffers, [`JvmtiString`] owns modified
//! UTF-8, and [`JvmtiStringList`] and [`LocalVariableTable`] free operation-specific
//! nested allocations before their outer arrays. [`Allocation`] uses the same
//! allocator ownership for newly allocated, zero-initialized bytes. All owners
//! borrow the originating environment and are neither `Send` nor `Sync`.
//!
//! Drop performs best-effort cleanup. The consuming `free` methods attempt every
//! owned deallocation once and return the first error, without retrying in drop.
//! Failed deallocations may leak. `into_raw` transfers all allocation ownership
//! when a caller needs its own cleanup/recovery policy or a VM ownership transfer.
//! The environment and VM must remain live through cleanup, and callers must
//! satisfy JVMTI thread-attachment and execution-context requirements.
//!
//! JVMTI allocations and JNI references are independent: freeing an array of
//! JNI handles neither deletes those references nor extends their validity. Use
//! JNI reference wrappers and local-frame lifetimes separately. Raw adoption
//! must never be used for callback-borrowed data or JNI string/array leases.
#![allow(unsafe_code, reason = "JVMTI utilities invoke the native function table.")]

mod api;
mod internal;

pub use abos_jvmti_sys as sys;
pub use api::*;
