# abos-jvmti

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jvmti-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jvmti)
[![crates.io](https://img.shields.io/crates/v/abos-jvmti?style=for-the-badge&labelColor=555555&color=00599C)](https://crates.io/crates/abos-jvmti)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/A-Box-of-Scraps/abos-jni-rs/cross-platform.yml?style=for-the-badge&labelColor=555555&label=CI)](https://github.com/A-Box-of-Scraps/abos-jni-rs/actions/workflows/cross-platform.yml)

Environment and agent utilities for the Java Virtual Machine Tool Interface.

## Installation

For an agent loaded into an existing JVM:

```toml
[dependencies]
abos-jvmti = "0.2.0"
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
abos-jvmti = "0.2.0"
```

Replace `src/lib.rs` with an agent that observes VM initialization:

```rust
use std::ffi::{c_char, c_void};
use std::io::Write;

use abos_jvmti::{Env, Result, catch_callback, sys};

unsafe extern "system" fn vm_init(
    _env: *mut sys::jvmtiEnv,
    _jni: *mut sys::JNIEnv,
    _thread: sys::jthread,
) {
    catch_callback((), || {
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
    catch_callback(sys::JNI_ERR, || {
        let result: Result<()> = (|| {
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
        })();
        if result.is_ok() { sys::JNI_OK } else { sys::JNI_ERR }
    })
}
```

`VMInit` needs no optional capability. Installing callbacks and enabling events are separate steps. The VM copies the
callback table, but the callback functions must remain executable. Dropping the Rust `Env` handle does not dispose the
native environment, so the registration remains active after `Agent_OnLoad` returns.

This example uses `catch_callback` at both native entry points and avoids panicking on output failures. Returning
`JNI_ERR` from `Agent_OnLoad` reports initialization failure to the VM. See the panic-boundary limitations below.

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

### Callback panic boundaries

`catch_callback(fallback, body)` contains unwinding Rust panics without attaching threads, accessing JNI/JVMTI, throwing
Java exceptions, logging, or installing panic hooks. `catch_callback_with(fallback, body, on_panic)` also protects a
caller-supplied panic handler and destruction of its unused captures. Both require `Copy` results, so no result/fallback
destructor runs outside the boundary.

Choose a fallback for the callback's contract, not a generic default:

| Callback                          | Typical panic fallback                             |
| --------------------------------- | -------------------------------------------------- |
| Event notification                | `()`; record failure for later reporting if needed |
| Modern heap visitor               | `sys::JVMTI_VISIT_ABORT`                           |
| Legacy heap visitor               | `sys::JVMTI_ITERATION_ABORT`                       |
| `Agent_OnLoad` / `Agent_OnAttach` | `sys::JNI_ERR`                                     |

Bodies, handlers, and their destructors must obey the original callback's context restrictions. For restricted
callbacks, prefer an atomic failure flag over synchronous logging or Java calls. Handle ordinary errors inside the body,
including any potentially panicking formatting/reporting. Construct captures without panicking and keep other
potentially panicking work inside the boundary. Publish output pointers and lengths only after fallible work succeeds;
catching a panic does not undo mutations or repair shared state.

Caught panic payloads are deliberately leaked: their destructors could panic again or execute context-forbidden code.
Repeated panics can leak memory, so these helpers are a last-resort FFI boundary, not ordinary error handling. They do
not recover from aborting panics, double panics, or foreign exceptions. The process-wide panic hook still runs before
catching the unwind and must be appropriate for the callback's context. An allocation-free or lock-free panic path is
not guaranteed.

### Explicit shutdown coordination

The
[shutdown agent](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jvmti/examples/shutdown_agent.rs)
demonstrates live-phase shutdown coordinated from an attached Java native call:

1. Close callback admission before disabling events and clearing callbacks.
2. Poll until admitted callbacks finish. A callback may request shutdown but must not wait for itself to drain.
3. Release environment-dependent resources, then dispose the environment once and retire all handles.

Real agents must also stop and join workers, account for per-thread event enables, release monitors, and resume
suspended threads. Disabling delivery does not wait for callbacks already in flight. Cleanup errors and panics are
terminal in this example: it leaves unreleased state pinned for VM teardown rather than risking a retry with partial
effects. A timeout never authorizes freeing resources still in use.

**This is not a library-unload protocol.** Keep the library, admission gate, and JNI entry points resident through VM
termination. A delayed callback can still enter native code after disposal; the permanently closed gate prevents it from
accessing the environment or freed resources. Never reopen or reuse that gate for another environment.

From the workspace root, with a JDK available:

```sh
cargo build --locked -p abos-jvmti --example shutdown_agent
javac -d target/shutdown-java crates/abos-jvmti/tests/java/ShutdownTarget.java
java -Xcheck:jni "-agentpath:$PWD/target/debug/examples/libshutdown_agent.so" -cp target/shutdown-java ShutdownTarget
```

Adjust Linux's library naming and absolute path for your platform. The driver exercises delivery and repeated shutdown
calls. Shutdown is not attempted from `VMDeath` or `Agent_OnUnload`.

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

## API reference

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
