# JVMTI TODO

## 1. JVM compatibility

- [x] Define and document the minimum supported JVMTI version and JDK test matrix.
- [x] Add a `JvmtiVersion` type using JVMTI's version masks and explicit-version environment constructors.
- [x] Reuse the JNI crate's version-specific function-table approach, with accurate JVMTI introduction-version metadata
      and preserved reserved slots.
- [x] Replace accesses that assume a complete JDK 25 table with version-appropriate accesses.
- [x] Query and retain the environment's actual version; report unsupported operations before accessing unavailable
      slots.
- [x] Handle function, event, and capability availability separately, including version-appropriate callback tables.
- [x] Account for each successful JVMTI `GetEnv` call creating an independent environment rather than upgrading an
      existing one.
- [x] Test compatibility against the baseline and newer JDKs, including unsupported-version and unavailable-feature
      paths.

## 2. Returned-data ownership

- [x] Add ownership wrappers for JVMTI-returned buffers, arrays, and modified UTF-8 strings, beyond newly allocated byte
      buffers.
- [x] Support nested allocations and operation-specific cleanup for returned structures.
- [x] Keep JVMTI allocation ownership separate from JNI reference ownership and lifetimes.
- [x] Use the originating environment's `Deallocate`, never Rust's allocator, for JVMTI-owned memory.
- [x] Document environment lifetime, thread attachment, phase, and callback-context requirements for access and cleanup.
- [x] Provide explicit ownership transfer and fallible cleanup where appropriate.
- [x] Test empty results, partial failures, nested cleanup, and exactly-once deallocation.

## 3. Missing Rust wrapper features

Higher-level wrappers are implemented alongside the raw bindings.

- [x] **Instrumentation:** class retransformation, class redefinition, and bytecode retrieval.
- [x] **Debugging:** breakpoints, per-thread stepping configuration, and local-variable access and modification.
- [x] **Threads:** stack traces, suspension and resumption, and virtual-thread operations.
- [x] **Heap:** object tags, heap traversal, and reference following.
- [x] **Synchronization:** raw-monitor creation, destruction, locking, waiting, and notification.
- [x] **Agent management:** environment-local storage, capability relinquishment, and per-thread event enabling.
- [x] Document each wrapper's version, capability, phase, callback-safety, thread, and ownership requirements; retain
      `unsafe` where these cannot be enforced.
- [x] Add real-JVM integration tests for each feature area, including error handling and resource cleanup.

## 4. `jvmticmlr.h` support

- [ ] Add raw bindings for compiled-method-load records, including record headers, record kinds, inline information, and
      PC-to-stack mappings.
- [ ] Clearly distinguish implementation-specific compiled-method metadata from the standard `jvmti.h` interface.
- [ ] Validate layouts and constants against the installed JDK's `jvmticmlr.h`.
- [ ] Add callback-scoped access to `CompiledMethodLoad` metadata, with explicit handling of null data, unknown record
      kinds, and unsupported record versions.
- [ ] Document metadata lifetimes and provide owned copies where data must outlive the callback.
- [ ] Add a profiling example and tests for record traversal and inline-frame decoding.
