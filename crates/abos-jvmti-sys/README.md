# abos-jvmti-sys

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jvmti--sys-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jvmti-sys)
[![crates.io](https://img.shields.io/crates/v/abos-jvmti-sys?style=for-the-badge&labelColor=555555&color=00599C)](https://crates.io/crates/abos-jvmti-sys)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Raw Rust definitions corresponding to `jvmti.h`.

## Installation

```toml
[dependencies]
abos-jvmti-sys = "0.1.0"
```

This `no_std` crate provides the JVMTI 25 declaration set and shares/re-exports JNI types from `abos-jni-sys`. Normal
Rust builds do not require JDK headers, a C compiler, or a running JVM. Using the native interfaces requires a
compatible JVM. The crate does not load, start, or automatically link one.

For environment management, returned-data owners, and a complete startup-agent example, see
[abos-jvmti](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/README.md). That crate
re-exports these declarations as `abos_jvmti::sys`.

## Quick Start

Given the VM pointer supplied to an agent, request a JVMTI 1.2 environment through the JNI invocation table:

```rust
use abos_jvmti_sys as sys;
use core::{ffi::c_void, ptr};

/// # Safety
/// `vm` must point to a live VM supporting the JNI 1.2 invocation table.
/// Call during OnLoad or the live phase on a thread permitted to use GetEnv.
pub unsafe fn create_environment(vm: *mut sys::JavaVM) -> Result<*mut sys::jvmtiEnv, sys::jint> {
    let mut raw: *mut c_void = ptr::null_mut();
    // SAFETY: The caller guarantees the VM and context; raw is a writable output.
    let status = unsafe { ((**vm).v1_2.GetEnv)(vm, &raw mut raw, sys::JVMTI_VERSION_1_2) };
    if status == sys::JNI_OK {
        Ok(raw.cast())
    } else {
        Err(status)
    }
}
```

`GetEnv` returns a JNI status code. Passing a **JVMTI** version creates an independent JVMTI environment; it is not the
same operation as querying the current thread's JNI environment. Each successful request creates another connection,
with its own capabilities and callbacks. Keep it alive while in use and arrange explicit `DisposeEnvironment` cleanup
when no callbacks, resources, or other users remain.

`jvmtiEnv` is a table-pointer alias (`*const jvmtiInterface_1_`), so native operations receive `*mut jvmtiEnv`. This
example does not export an agent entry point or attach a JNI thread; those responsibilities remain with the caller.

## Scope and limitations

### Declarations and native errors

The crate exposes JVMTI constants, C-layout structures and unions, function-table entries, callback types, capability
bitfields, and compatibility helpers. JNI object handles, IDs, and VM/environment pointer types use the same definitions
as `abos-jni-sys`.

JVMTI functions and callback entries are nullable `Option<unsafe extern ... fn(...)>` values. Check for `Some` before
calling, even after selecting a supported table view. Reserved slots must never be called. Table operations generally
return `jvmtiError`; compare success with `JVMTI_ERROR_NONE`, not the JNI invocation status constants.

### Interface versions and table views

`jvmtiInterface_1_` (also aliased as `jvmtiInterface_1`) retains the complete raw JVMTI 25 table. Unlike the JNI sys
table, it is a struct, not a union. Bounded historical views are separate structs:

| View                    | Interface version |
| ----------------------- | ----------------- |
| `jvmtiInterface_1__1_2` | 1.2               |
| `jvmtiInterface_1__9`   | 9                 |
| `jvmtiInterface_1__11`  | 11                |
| `jvmtiInterface_1__21`  | 21                |
| `jvmtiInterface_1__25`  | 25                |

Views preserve slot offsets, replace unavailable interior entries with reserved pointers, and stop at the last available
slot. Establish the required version before using a view. Never create a reference to the full JVMTI 25 table over a
shorter older environment's table, or copy more table bytes than the runtime provides.

`JVMTI_VERSION` denotes the declaration set's version 25, not the running environment's actual version. Request the
minimum interface you need, then use `GetVersionNumber` to query the environment. JNI and JVMTI version identifiers are
not interchangeable. There is no automatic version negotiation or fallback.

### Capabilities and callback compatibility

Initialize `jvmtiCapabilities` with `Default::default()` and use named getters/setters, such as
`set_can_tag_objects(true)`. This keeps reserved bits zero and handles bit placement for the target's C ABI. Setting a
bit only prepares a request; capabilities must still be acquired through the VM.

The compatibility helpers describe the supported binding surface with a JVMTI 1.2 baseline:

| Helper                                           | Meaning                                                             |
| ------------------------------------------------ | ------------------------------------------------------------------- |
| `jvmtiCapabilities::required_version()`          | Minimum supported version for the selected capability bits          |
| `jvmtiEventCallbacks::required_version()`        | Minimum supported version for the populated callback entries        |
| `jvmtiEventCallbacks::size_for_version(version)` | Known callback-table prefix size for a valid JVMTI version          |
| `jvmti_event_version(event)`                     | Minimum supported event version, or `None` for unknown/reserved IDs |

These helpers make no VM calls. They do not validate an arbitrary version integer, grant capabilities, or establish
availability in the current phase. Before registering callbacks, check their required version against the environment's
actual version and use the appropriate prefix size. Keep reserved callbacks null. Installing callback pointers does not
enable event delivery; notification modes must be configured separately.

### Execution context and ownership

Raw declarations do not enforce VM phases, thread attachment, capabilities, callback restrictions, or reference
lifetimes. A non-null function pointer is not permission to call it from every context. Callbacks can be concurrent and
reentrant; synchronize state and never unwind into the VM.

Structures containing pointers are not owned Rust collections. Release JVMTI allocations with that environment's
`Deallocate`, including operation-specific nested allocations where required. JNI references have separate cleanup and
local-frame rules. Never pass Rust allocations, callback-borrowed memory, or JNI string/array leases to `Deallocate`.

### Compiled-method metadata

The `jvmtiCompiledMethodLoad*` records, `PCStackInfo`, and `JVMTI_CMLR_*` constants describe the implementation-specific
`jvmticmlr.h` format. Standard JVMTI does not guarantee that a VM uses this representation for `compile_info`.

Establish format compatibility before interpreting these pointers. Record versions are independent of the JVMTI
interface version. Records and nested arrays expire when `CompiledMethodLoad` returns; copying a raw struct only copies
its pointers. Deep-copy data that must outlive the callback, and remember that method IDs and code addresses do not keep
classes or compiled code alive.

## Examples

### Query an environment's actual version

Read only the baseline table entry rather than forming a full-table reference:

```rust
use abos_jvmti_sys as sys;

/// # Safety
/// `env` must be a live JVMTI 1.2-or-newer environment. The calling thread and
/// context must permit GetVersionNumber; do not use this in restricted callbacks.
pub unsafe fn environment_version(env: *mut sys::jvmtiEnv) -> Result<sys::jint, sys::jvmtiError> {
    // SAFETY: The caller guarantees the environment and baseline table entry.
    let get_version = unsafe {
        let table = (*env).cast::<sys::jvmtiInterface_1__1_2>();
        (*table).GetVersionNumber
    }.ok_or(sys::JVMTI_ERROR_NOT_AVAILABLE)?;
    let mut version = 0;
    // SAFETY: The context is valid and version is a writable output.
    let error = unsafe { get_version(env, &raw mut version) };
    if error == sys::JVMTI_ERROR_NONE {
        Ok(version)
    } else {
        Err(error)
    }
}
```

The example reports an absent function pointer as `JVMTI_ERROR_NOT_AVAILABLE`. A successful call returns the actual
version; it need not equal the minimum requested during environment creation.

### Prepare capabilities and callback metadata

Build request records without making native calls:

```rust
use abos_jvmti_sys as sys;

fn main() {
    let mut capabilities = sys::jvmtiCapabilities::default();
    capabilities.set_can_tag_objects(true);
    assert!(capabilities.can_tag_objects());
    assert_eq!(capabilities.required_version(), sys::JVMTI_VERSION_1_2);

    let callbacks = sys::jvmtiEventCallbacks::default();
    assert!(callbacks.VMInit.is_none());
    assert_eq!(callbacks.required_version(), sys::JVMTI_VERSION_1_2);
    let prefix = sys::jvmtiEventCallbacks::size_for_version(sys::JVMTI_VERSION_1_2)
        .expect("known baseline version");
    assert!(prefix <= core::mem::size_of::<sys::jvmtiEventCallbacks>());
    assert_eq!(
        sys::jvmti_event_version(sys::JVMTI_EVENT_VIRTUAL_THREAD_START),
        Some(sys::JVMTI_VERSION_21),
    );
}
```

A default callback table selects no callbacks. In an agent, populate the callbacks you need and use the environment's
queried version rather than assuming that a constant proves runtime support.

## Tests and API reference

From a repository checkout, run the JDK-independent layout and compatibility checks:

```sh
cargo test --locked --package abos-jvmti-sys
```

For ABI validation against the JVMTI 25 headers, set `JAVA_HOME` to a host JDK 25 installation and provide a native C
compiler. The standalone header probe is not a cross-compilation test:

```sh
cargo test --locked --manifest-path tests/abos-jvmti-sys-systest/Cargo.toml
```

The checked-in declarations can also be checked against the local JDK 25 header with Python and `rustfmt` installed:

```sh
python scripts/generate_jvmti.py --check
```

See the
[interface declarations](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/src/api/interface.rs),
[callback declarations](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/src/api/callbacks.rs),
and [header probe](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/tests/abos-jvmti-sys-systest) for the full
API and ABI checks. Generate the Rust reference locally with:

```sh
cargo doc --package abos-jvmti-sys --no-deps --open
```

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
