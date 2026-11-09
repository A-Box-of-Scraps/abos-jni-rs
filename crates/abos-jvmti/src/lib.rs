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
