# abos-jvmti

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jvmti-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jvmti)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)

Environment and agent utilities for the Java Virtual Machine Tool Interface.

## Installation

For an agent loaded into an existing JVM:

```toml
[dependencies]
abos-jvmti = "0.1.0"
```

The crate targets JVMTI 1.2 or newer (Java 8+) and interoperates with
[abos-jni](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/README.md). It does not install Java
or start a JVM by default. Add `abos-jni` separately when using its JNI wrappers in your agent.

The optional `invocation` feature forwards `abos-jni`'s desktop JVM invocation support. It is not needed for agents
loaded by Java. Raw JVMTI declarations are re-exported as `abos_jvmti::sys`; see
[abos-jvmti-sys](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti-sys/README.md) for the
native API.

## Quick Start

### Build a startup agent

Create a Rust library:

```sh
cargo new --lib jvmti-hello
cd jvmti-hello
```

Configure `Cargo.toml`:

```toml
[package]
name = "jvmti-hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
abos-jvmti = "0.1.0"
```

Replace `src/lib.rs` with an agent that observes VM initialization:

```rust
use std::ffi::{c_char, c_void};
use std::io::Write;
use std::panic::catch_unwind;

use abos_jvmti::{Env, Result, sys};

unsafe extern "system" fn vm_init(
    _env: *mut sys::jvmtiEnv,
    _jni: *mut sys::JNIEnv,
    _thread: sys::jthread,
) {
    let _ = catch_unwind(|| {
        let _ = writeln!(std::io::stderr().lock(), "JVMTI agent initialized");
    });
}

/// # Safety
/// The JVM must supply a live VM pointer during OnLoad and keep this library
/// loaded while its registered callbacks can execute.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Agent_OnLoad(
    vm: *mut sys::JavaVM,
    _options: *mut c_char,
    _reserved: *mut c_void,
) -> sys::jint {
    let result = catch_unwind(|| -> Result<()> {
        // SAFETY: The JVM supplies the live VM and permits environment creation
        // and event configuration during OnLoad. Callback code stays loaded.
        unsafe {
            let env = Env::from_raw_vm(vm)?;
            let callbacks = sys::jvmtiEventCallbacks {
                VMInit: Some(vm_init),
                ..Default::default()
            };
            let setup = env.set_event_callbacks(&callbacks)
                .and_then(|()| env.set_event_enabled(sys::JVMTI_EVENT_VM_INIT, true));
            if let Err(error) = setup {
                // VMInit cannot run before this OnLoad returns; no other users exist.
                let _ = env.dispose();
                return Err(error);
            }
        }
        Ok(())
    });
    match result {
        Ok(Ok(())) => sys::JNI_OK,
        _ => sys::JNI_ERR,
    }
}
```

`VMInit` needs no optional capability. Installing callbacks and enabling events are separate steps. The VM copies the
callback table, but the callback functions must remain executable. Dropping the Rust `Env` handle does not dispose the
native environment, so the registration remains active after `Agent_OnLoad` returns.

This example catches unwinding panics at both native entry points and avoids panicking on output failures. It does not
recover from `panic = "abort"`. Returning `JNI_ERR` from `Agent_OnLoad` reports initialization failure to the VM.

### Load the agent from Java

Create `HelloAgent.java` in the Cargo package root:

```java
public class HelloAgent {
    public static void main(String[] args) {
        System.out.println("Hello from Java");
    }
}
```

With a JDK's `javac` and `java` available, build and launch:

```sh
cargo build
javac -d target/java HelloAgent.java
java "-agentpath:$PWD/target/debug/libjvmti_hello.so" -cp target/java HelloAgent
```

These commands use a POSIX shell, Linux's `.so` suffix, and Cargo's default target directory. On macOS, use
`libjvmti_hello.dylib`; on Windows, pass the absolute path to `jvmti_hello.dll` using your shell's path syntax.

The agent prints `JVMTI agent initialized` to stderr, and the Java application prints `Hello from Java` to stdout.
Agents are loaded with `-agentpath`, not by declaring a Java native method or calling `System.loadLibrary`. This example
exports `Agent_OnLoad`; it does not provide an `Agent_OnAttach` entry point for dynamic attachment.

## Scope and limitations

### Environments and versions

`Env` is a borrowed, `Copy`, cross-thread handle, not a JNI attachment guard. Copies share the same native environment,
capabilities, and callbacks. Each successful `from_vm(...)` or `from_raw_vm(...)` call instead creates an independent
JVMTI environment.

Default constructors request the JVMTI 1.2 baseline once. Explicit-version constructors require the requested minimum
without silently downgrading it. `Env::version()` returns the actual version retained at construction without another VM
call. JNI and JVMTI version identifiers are not interchangeable.

`from_raw(...)` queries an existing environment's version. In restricted callbacks where that query is forbidden, use
`from_raw_with_version(...)` with the environment's previously queried actual version, not just the version originally
requested. Keep the environment and VM alive for every borrowed handle and resource.

### Capabilities, events, and execution context

Request the capabilities needed by your operations, install callbacks, then enable their events. Availability depends on
the VM and phase; potential capabilities are a snapshot, not a guarantee that acquisition will succeed. Registering a
callback does not grant its capability or enable event delivery.

Newer capabilities, callbacks, events, and version-specific operations are checked against the environment version.
Other availability and execution-context errors remain native JVMTI errors. Read each operation's `Safety` contract:
phase, thread attachment, callback restrictions, and reference validity still matter, so VM operations remain unsafe. In
particular, heap, GC, and `ObjectFree` callbacks cannot perform arbitrary JNI/JVMTI work.

Callbacks can be concurrent and reentrant. Synchronize agent state and never unwind into the VM. Replacing a callback
table or disabling an event is not a substitute for coordinating with callbacks already in flight.

### Supported operations

The wrappers cover instrumentation and bytecodes, breakpoints and stepping, typed local-variable access, stack traces,
thread suspension/resumption, object tags, heap traversal, event filtering, capabilities, and environment-local storage.
Bulk virtual-thread suspension and resumption require JVMTI 21 or newer.

Raw declarations remain available through `sys` when an operation has no wrapper. This crate does not make an arbitrary
JVMTI operation safe merely because the environment pointer is valid.

### Returned data and JNI references

`Allocation`, `JvmtiArray`, `JvmtiString`, `JvmtiStringList`, and `LocalVariableTable` own JVMTI allocations and borrow
the originating environment. Returned-data owners are neither `Send` nor `Sync`. Operation-specific nested owners free
child allocations before their outer arrays; a flat array owner does not own arbitrary pointees.

Drop attempts cleanup once and ignores errors. Use consuming `free()` methods to observe failures, or `into_raw()` to
transfer ownership and take responsibility for cleanup. Failed cleanup is not retried in drop and may leak. The VM,
environment, and permitted cleanup context must remain valid until resources are released.

JVMTI allocation ownership is separate from JNI reference ownership. Freeing an array containing JNI handles does not
delete those references or extend their validity. Manage JNI frames and references separately with `abos-jni`. Never
adopt callback-borrowed data or JNI string/array leases as owned JVMTI allocations.

### Monitors, storage, and disposal

`RawMonitor` owns a recursive native monitor, and its guard tracks one entry on the acquiring thread. Explicit
`unlock()` and `destroy()` report failures; drop attempts cleanup once. Raw monitors and environment-local storage do
not establish safe Rust data synchronization or manage the lifetime of your stored state automatically.

`Env` is not disposed on drop. Before explicit `dispose()`, coordinate with all callbacks and handle copies, release
allocations and monitors, and resume suspended threads. No remaining user may access the environment after disposal.

### Compiled-method profiling

`CompiledMethodRecords` decodes callback-borrowed `compile_info` using the implementation-specific `jvmticmlr.h` ABI.
Establish VM format compatibility before decoding; this is not a portable standard-JVMTI payload. The profiling example
requires an explicit `hotspot` option to assert that compatibility.

Recognized records can be deep-copied into Rust-owned storage, but copied method IDs and addresses do not keep classes
or compiled code alive. Track code unloading and resolve symbols while their owners are valid. Callback metadata must
never be passed to JVMTI allocation owners or `Deallocate`.

## Examples

### Request an optional capability

Check whether object tagging could be acquired, then request it. A false result means it was not advertised; acquisition
errors are still returned:

```rust
use abos_jvmti::{Env, Result, sys};

/// # Safety
/// Call during OnLoad or the live phase on a JVMTI-permitted thread, outside
/// restricted callbacks. Coordinate capability changes with other users.
pub unsafe fn request_tagging(env: &Env<'_>) -> Result<bool> {
    // SAFETY: The caller supplies the capability-management context.
    unsafe {
        if !env.potential_capabilities()?.can_tag_objects() {
            return Ok(false);
        }
        let mut requested = sys::jvmtiCapabilities::default();
        requested.set_can_tag_objects(true);
        env.add_capabilities(&requested)?;
    }
    Ok(true)
}
```

### Allocate and explicitly release JVMTI memory

Use the JVMTI allocator when an operation requires memory with JVMTI ownership:

```rust
use abos_jvmti::{Env, Result};

/// # Safety
/// The thread must be permitted to allocate and deallocate JVMTI memory, and
/// the environment and VM must remain live through cleanup.
pub unsafe fn allocate_marker(env: &Env<'_>) -> Result<()> {
    // SAFETY: The caller guarantees the allocation and cleanup context.
    let mut bytes = unsafe { env.allocate(4)? };
    bytes.copy_from_slice(b"JVMT");
    bytes.free()
}
```

### Complete agents

The [events agent](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/examples/events_agent.rs)
demonstrates capability requests, VM/thread callbacks, and allocation ownership. Its optional `features` mode adds
instrumentation, debugging, heap, and monitor exercises for the bundled Java fixtures.

The
[profiling agent](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/examples/profiling_agent.rs)
demonstrates compiled-method events and inline metadata on a compatible HotSpot VM. Build either example as a shared
library, rather than running it as a Rust executable:

```sh
cargo build --package abos-jvmti --example events_agent
cargo build --package abos-jvmti --example profiling_agent
```

Load the resulting library from `target/debug/examples` using an absolute `-agentpath`. See the
[agent integration test](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/tests/agent.rs) for
matching Java fixtures, startup options, and compatibility checks.

## Tests and API reference

From a checkout, run the mock-environment tests, ownership checks, and real-agent integration test:

```sh
cargo test --locked --package abos-jvmti
```

The real-agent test requires `JAVA_HOME` pointing to a compatible HotSpot JDK. It builds agents, compiles Java fixtures,
and launches separate JVM processes. `JVMTI_TEST_JAVA_HOMES` can select additional runtimes using the platform's
path-list separator; the profiling checks require those VMs to implement the expected HotSpot metadata ABI too.

Every operation documents its context and ownership requirements in rustdoc. Generate the reference with:

```sh
cargo doc --package abos-jvmti --no-deps --open
```

## Changelog

All notable changes to this crate are documented
[HERE](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/CHANGELOG.md).

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
