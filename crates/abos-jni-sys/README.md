# abos-jni-sys

[![github](https://img.shields.io/badge/github-A--Box--of--Scraps/abos--jni--rs-EE7C37?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/crates/abos-jni-sys)
[![MIT license](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge&labelColor=555555)](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE)

Raw Rust definitions corresponding to jni.h.

## Installation

```toml
[dependencies]
abos-jni-sys = "0.1.0"
```

This is a `no_std` crate containing raw JNI declarations. Building it does not require a JDK or `jni.h`. Calls through
the JNI tables require a live JVM and valid JNI pointers.

For checked calls, reference lifetimes, thread attachment, and JVM discovery, use
[abos-jni](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni/README.md) instead. That crate also
re-exports these declarations as `abos_jni::sys`.

## Quick Start

Given a JNI environment supplied by a JVM, read its version through the baseline table view:

```rust
use abos_jni_sys as sys;

/// # Safety
/// `env` must be a valid JNI environment for the current attached thread,
/// with a live function table and no pending Java exception.
pub unsafe fn jni_version(env: *mut sys::JNIEnv) -> sys::jint {
    // SAFETY: The caller supplies a live environment; GetVersion is in JNI 1.1.
    unsafe { ((**env).v1_1.GetVersion)(env) }
}
```

This function does not create a JVM or attach a thread. It must be called with an environment obtained from a native
method or the invocation API, not a null or fabricated pointer.

`JNIEnv` is itself a pointer alias (`*const JNINativeInterface_`), so JNI functions receive `*mut JNIEnv`. `JavaVM` is
likewise a table-pointer alias, and invocation functions receive `*mut JavaVM`. The two dereferences in the example
access the environment's function table; they are not an extra layer to remove.

## Scope and limitations

### Raw declarations and values

The crate exposes JNI scalar aliases, object and array handles, field/method IDs, `jvalue`, constants, native-method
registration records, and VM initialization/attachment structures. Names follow `jni.h`.

| Type                                          | Representation                                                            |
| --------------------------------------------- | ------------------------------------------------------------------------- |
| `jint`, `jlong`                               | `i32`, `i64`                                                              |
| `jboolean`                                    | `u8`, not Rust `bool`; pass `JNI_FALSE` or `JNI_TRUE` to Java             |
| `jchar`                                       | A UTF-16 code unit (`u16`), not Rust `char`                               |
| `jobject`, `jstring`, `jclass`, array handles | Raw object-reference pointers, not owned Rust wrappers                    |
| `jvalue`                                      | A C-layout union whose member must match the Java argument or result type |

Object aliases do not distinguish local, global, or weak references in Rust's type system. There is no automatic
reference cleanup, thread checking, or Java exception conversion.

### Versioned function tables

`JNINativeInterface_` and `JNIInvokeInterface_` are unions of version-specific views, not flat structs with every method
available directly. Select a view supported by the running JVM:

| Table                 | Union fields                                       |
| --------------------- | -------------------------------------------------- |
| `JNINativeInterface_` | `v1_1`, `v1_2`, `v1_4`, `v1_6`, `v9`, `v19`, `v24` |
| `JNIInvokeInterface_` | `v1_1`, `v1_2`, `v1_4`                             |

Each view exposes functions available at that version while preserving their JNI slot offsets. Named types such as
`JNINativeInterface__1_1` and `JNIInvokeInterface__1_2` are also exported. Views are created for table changes, not for
every `JNI_VERSION_*` constant: for example, there is no `v1_8` field.

Selecting a union field does not check the runtime version. Establish support before accessing newer entries; the
presence of a version constant in Rust is not evidence that the running JVM implements it. Borrow the supported view
rather than copying the full union from a possibly shorter, older JVM table. Reserved slots are not callable APIs.

Function entries are `unsafe extern` function pointers, not `Option` values. The native table includes JNI 24's
`GetStringUTFLengthAsLong` in `v24`; using that view requires a VM supporting the corresponding table.

### Invocation and linking

The crate declares `JNI_CreateJavaVM`, `JNI_GetCreatedJavaVMs`, and `JNI_GetDefaultJavaVMInitArgs`, but does not
implement, locate, load, or automatically link a JVM library. Calling those symbols requires your application to arrange
native linking or resolve the entry points dynamically.

When a JVM already supplies an environment or VM pointer, calls through its function tables do not require resolving
those global entry-point symbols. Use `abos-jni` with `invocation` enabled if you want managed desktop JVM loading.

### Safety and resource management

Every raw call must satisfy the JNI contract: use an environment belonging to the current attached thread, valid live
references, matching method/field IDs and signatures, and correctly sized buffers. A null-pointer check alone does not
establish those requirements.

Check JNI status codes, null results, and pending Java exceptions as required by the called function. A returned
primitive value is not proof that a call succeeded. Manage local/global references and release acquired strings and
array buffers using the corresponding JNI operations. Do not let Rust panics unwind across a native-call boundary.

String APIs taking UTF-8 use JNI modified UTF-8, not ordinary Rust UTF-8. The ASCII C-string literals in typical JNI
names and descriptors need no conversion, but arbitrary Rust strings must be encoded appropriately. Prefer the
higher-level crate's JNI string helpers rather than casting Rust string buffers.

### Variadic calls

The declarations include JNI's variadic, `V` (`va_list`), and `A` (`jvalue` array) call forms. `va_list` uses
`core::ffi::VaList`, preserving target-specific layout and calling conventions. The `A` forms are often simpler for Rust
callers because arguments can be supplied as an explicit `jvalue` array.

## Examples

### Query an existing thread attachment

`GetEnv` queries the current thread; it does not attach a detached thread:

```rust
use abos_jni_sys as sys;
use core::ptr;

/// # Safety
/// `vm` must point to a live JVM supporting JNI 1.2 or later.
pub unsafe fn current_env(vm: *mut sys::JavaVM) -> Result<*mut sys::JNIEnv, sys::jint> {
    let mut env: *mut sys::JNIEnv = ptr::null_mut();
    // SAFETY: The caller guarantees the VM supports this view; the output pointer
    // is writable and has the representation expected by GetEnv.
    let status = unsafe { ((**vm).v1_2.GetEnv)(vm, (&raw mut env).cast(), sys::JNI_VERSION_1_2) };
    if status == sys::JNI_OK {
        Ok(env)
    } else {
        Err(status)
    }
}
```

A detached thread produces `JNI_EDETACHED`. Attach it through the invocation table before making environment calls, and
arrange for it to be detached when appropriate.

### Pass explicit method arguments

For a static Java method with descriptor `(II)I`, populate the integer members of `jvalue`:

```rust
use abos_jni_sys as sys;

/// # Safety
/// `env` must belong to the current attached thread, with no pending exception.
/// `class` and `method` must be valid, matching handles for a static `(II)I` method.
pub unsafe fn call_two_ints(
    env: *mut sys::JNIEnv,
    class: sys::jclass,
    method: sys::jmethodID,
    left: sys::jint,
    right: sys::jint,
) -> sys::jint {
    let args = [sys::jvalue { i: left }, sys::jvalue { i: right }];
    // SAFETY: The caller guarantees matching handles and signature; the argument
    // array has two correctly typed values and lives through the call.
    unsafe { ((**env).v1_1.CallStaticIntMethodA)(env, class, method, args.as_ptr()) }
}
```

Inspect the environment for a Java exception after the call before treating the returned integer as a successful result.

## Tests and API reference

From a repository checkout, run the layout and compile-fail tests:

```sh
cargo test --locked --package abos-jni-sys
```

The raw JVM integration test is opt-in and requires `JAVA_HOME` pointing to a host JDK with its platform JVM library:

```sh
cargo test --locked --package abos-jni-sys --test invocation -- --ignored
```

See the
[integration test](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys/tests/invocation.rs) for
dynamic loading, VM creation, and raw JNI calls. The declarations for
[JNI functions](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys/src/api/native.rs) and
[invocation functions](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/crates/abos-jni-sys/src/api/invocation.rs)
define the available API. Generate the full Rust reference locally with:

```sh
cargo doc --package abos-jni-sys --no-deps --open
```

## License

Licensed under the [MIT License](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/LICENSE) by A Box of Scraps.

This project incorporates code from third-party projects. See:

- [licenses/](https://github.com/A-Box-of-Scraps/abos-jni-rs/tree/HEAD/licenses/): for their respective licenses.
- [THIRD_PARTY_NOTICES](https://github.com/A-Box-of-Scraps/abos-jni-rs/blob/HEAD/THIRD_PARTY_NOTICES): for attribution
  and additional notices.
