# Fuzzing

This isolated workspace fuzzes class-file parsing, binding metadata conversion, JNI descriptors, string encoding, and
JVMTI metadata decoding and returned-data ownership. It has its own nightly toolchain and lockfile; normal workspace
builds keep their existing toolchain and dependencies. The targets do not start a JVM. Building the workspace requires a
JDK because jbindgen embeds a compiled Java parser, even though the fuzz targets only exercise its Rust bytecode
parsers.

## Quick start

From the repository root, with mise and rustup installed:

```sh
mise run setup:fuzz
mise run test:fuzz-seeds
mise run fuzz:build
mise run --jobs 2 fuzz:smoke:all
```

The setup task installs the cargo-fuzz version pinned in `mise.toml` and activates the toolchain pinned in
`fuzz/rust-toolchain.toml`, installing missing components. It does not change your default toolchain. A C++11-capable
compiler is required to build libFuzzer. On GNU/Linux, the repository's linker configuration also requires mold (`mise
install mold`). Install the project JDK with `mise install java` if it is not already available.

`fuzz:build` builds all eight targets as an optional cache warm-up. Each smoke task waits for the seed task, builds only
its selected target through `cargo fuzz run`, then fuzzes for roughly 60 seconds, excluding compilation. Previously
built targets reuse the build cache. The initial limits per process are 64 KiB generated inputs, five seconds per input,
and a 2048 MiB RSS limit. These are operating limits, not format guarantees.

`fuzz:smoke:all` schedules the eight independent smoke tasks concurrently up to mise's job limit. `--jobs 2` runs at
most two at once (roughly four minutes of fuzzing); `--jobs 8` can run all eight at once (roughly one minute). Memory
limits apply separately to each process: eight workers can consume approximately 16 GiB in total, plus build and system
overhead. Use `--jobs 1` for sequential execution. A failure makes the task fail and, by default, stops the other
running tasks. There are no additional libFuzzer workers inside these tasks.

`fuzz:smoke` remains an alias for the cafebabe-only smoke test. To run a particular new target, use its task from the
table below. For example:

```sh
mise run fuzz:smoke:jni-signatures
```

For an ongoing local session:

```sh
mise run fuzz:cafebabe
```

Stop with Ctrl-C. Interesting inputs are saved as the session runs; later sessions reuse them. Fuzzing tasks are
deliberately separate from `mise run test` and ordinary workspace checks. The root formatting tasks include the fuzz
workspace.

## Targets and properties

| Target                  | Smoke task suffix       | Checks                                                                                                                                                         |
| ----------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cafebabe_parse_class`  | `cafebabe`              | Arbitrary class-file bytes must not crash the metadata parser                                                                                                  |
| `jbindgen_parse_class`  | `jbindgen-parse-class`  | Class metadata, member descriptors, and binding metadata conversion must not crash                                                                             |
| `jbindgen_descriptors`  | `jbindgen-descriptors`  | `parse_type_descriptor` and `parse_method_descriptor` must handle arbitrary UTF-8 inputs without crashing                                                      |
| `jni_signatures`        | `jni-signatures`        | `JavaType::from_str`, `RuntimeFieldSignature::from_str`, and `RuntimeMethodSignature::from_str`; accepted field/method signatures preserve their original text |
| `jni_string_roundtrip`  | `jni-string-roundtrip`  | `JNIString::new` output validates through `JNIStr::from_cstr`, and both views decode to the original Rust string                                               |
| `jni_mutf8`             | `jni-mutf8`             | Arbitrary bytes go through safe `CString` construction and `JNIStr::from_cstr`; accepted strings retain their bytes and can be decoded without crashing        |
| `jvmti_compiled_method` | `jvmti-compiled-method` | Synthetic graphs match a traversal model; cycles/errors terminate, unsupported payloads remain unread, and deep copies outlive backing storage                 |
| `jvmti_returned_data`   | `jvmti-returned-data`   | Mock returned arrays preserve values and ownership; cleanup order, failures, missing functions, and raw transfers match an allocation registry                 |

Each task name is `fuzz:smoke:` followed by its suffix. Each target has its own working corpus and artifacts directory.
Multiple function calls inside one harness run sequentially on the same input; concurrency is between target processes.

The text targets skip non-UTF-8 bytes because their APIs take Rust strings. The modified UTF-8 target instead accepts
raw bytes and lets `CString::new` reject interior NULs. No unchecked constructor is given unvalidated bytes. The
validator accepts unpaired surrogate code units, whose decoding may be lossy; the harness deliberately does not demand
that such bytes survive a decode/encode round trip. The JNI and jbindgen descriptor parsers have different validation
policies, so their acceptance decisions are not treated as equivalent.

Jbindgen's private parsers are exposed through the hidden `fuzzing` module only with the opt-in `_fuzzing` feature. This
is internal harness access, not a supported public API.

Returning `Err` or `None` for invalid inputs is expected. Panics, crashes, sanitizer reports, and resource-limit
failures need investigation. The harnesses do not catch panics. Cargo-fuzz's default AddressSanitizer configuration is
used.

LibFuzzer mutates inputs and keeps examples that exercise interesting execution paths. Seeds help it reach beyond
initial format checks. The parser targets primarily check robustness, not full format correctness or JVM bytecode
validity; the string target also checks round-trip correctness. A clean run is not proof that there are no bugs.
Deterministic regression tests remain necessary.

## JVMTI harnesses

These targets exercise Rust wrappers, not a JVM implementation. Shared harness code in `src/` is also called by the seed
tests. Inputs describe bounded structures; they are never cast directly to native records or function pointers.

`jvmti_compiled_method` builds up to 16 stable boxed records, with at most eight PCs per inline record and eight frames
per PC. Links select record indices or null, including self-cycles and longer cycles. Unknown kinds and unsupported
versions use header-only allocations, making accidental payload reads detectable by AddressSanitizer. All dereferenced
non-null pointers have aligned, initialized backing storage; positive array counts fit their allocations. Negative
counts and null arrays exercise explicitly supported errors. The oracle compares record contents, frame order, the first
error, and termination, then checks owned copies after freeing the graph and nested arrays. Opaque PC addresses and
method IDs are copied but never dereferenced.

`jvmti_returned_data` uses a per-input mock environment with up to eight entries and a 128-byte text payload. Strings
are encoded with `JNIString`, so invalid MUTF-8 is never passed to an unchecked owner. It varies null and allocated
empty outputs, negative counts, undefined native-error outputs, missing functions, null required children, optional
generic signatures, and a selected deallocation failure. The registry checks every cleanup attempt in child-before-outer
order, rejects unknown/double frees, checks that cleanup continues after errors without retrying, and verifies that
`into_raw` frees nothing. Failed frees and transferred allocations are reclaimed only after the assertions.
`test:fuzz-seeds` also runs a deterministic matrix covering operations, modes, ownership actions, missing functions, and
every cleanup failure position for an eight-entry local-variable table.

### Binary input layout

Missing bytes read as zero; multi-byte integers are little-endian. Selector bytes are reduced modulo the number of
choices, and unused trailing bytes are ignored.

- Compiled methods start with record count (modulo 17) and starting index (modulo count + 1; count denotes null). Each
  record has a kind selector and next index. Kind 0 is an unknown header with one additional kind byte; kind 1 is an
  unsupported header with a recognized-kind selector and two i32 version components; kind 2 has 50 diagnostic bytes;
  kinds 3 and 4 are inline records. Inline records have a mode (normal, negative count, null array, empty null array)
  and PC count (modulo 9). Normal records then contain PCs: frame mode (normal, negative count, null methods, null BCIs,
  zero count), frame count (modulo 9), u32 PC, and method-u32/BCI-i32 pairs. The frame mode can override the declared
  count without changing the bounded backing allocation.
- Returned data starts with eight selector bytes: operation (properties, locals, bytecodes), output mode (normal, empty
  null, empty allocated, negative count, native error, null output, null child), action (drop, free, transfer), missing
  function (none, deallocator, query), failing cleanup attempt (one-based; zero disables), entry count (modulo eight
  plus one), bad-child index, and optional-generic-signature bitmask. The bitmask's low bit also selects whether a
  malformed local entry lacks its name or signature. Remaining bytes supply text and bytecode data. Null-child mode acts
  like normal mode for flat bytecode arrays.

## Seeds and generated files

The three committed files in `seeds/cafebabe_parse_class/` are hand-assembled, Java 8-version (major 52) class-file
metadata fixtures, not copied from a JDK or third-party archive:

| File            | Contents                                                                                                                                                             |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `minimal.class` | `java/lang/Object`, no superclass or members                                                                                                                         |
| `members.class` | A class implementing `Runnable`, a static field, a native method, and opaque attributes on the class and members                                                     |
| `unicode.class` | An accented class name, a supplementary-character field name encoded as a modified UTF-8 surrogate pair, and an unused string constant containing modified UTF-8 NUL |

These parser fixtures are shared by the cafebabe and jbindgen class targets, not JVM-loadability tests.

Additional hand-authored seeds are kept in:

- `seeds/descriptors/`: shared by both descriptor targets; primitive, object, array, method and Unicode descriptors,
  4,096 nested array brackets, and malformed descriptors. Files have no trailing newline.
- `seeds/jni_string_roundtrip/`: ASCII, embedded NUL, and BMP/supplementary Unicode in standard UTF-8.
- `seeds/jvmti_compiled_method/`: mixed records, empty lists, cycles, negative counts, and null nested arrays.
- `seeds/jvmti_returned_data/`: each operation and ownership action, empty and malformed outputs, missing functions, and
  cleanup failure injection.
- `seeds/jni_mutf8/`: ASCII, modified UTF-8 NUL, paired and unpaired surrogates, truncated/overlong encodings, and a
  four-byte standard UTF-8 sequence that modified UTF-8 validation should reject. Files omit the C terminator; the
  harness appends it safely.

`tests/seeds.rs` checks expected acceptance, rejection, metadata, and string properties. When editing seeds, update the
assertions and run `mise run test:fuzz-seeds`.

`mise run fuzz:seed` copies missing seeds into each target's `corpus/<target>/` directory, without replacing existing
files or removing discoveries. Shared source seeds are copied into independent working corpora. If a committed seed
changes, remove only its corresponding named copies from the working corpora before seeding again.

These directories are ignored by Git:

- `corpus/`: accumulated inputs used for mutation; reusable between sessions.
- `artifacts/`: saved inputs that triggered a failure; preserve useful findings before cleaning.
- `coverage/`: generated coverage output, if requested separately.
- `target/`: build output.

## Reproduce and minimize a finding

Run direct cargo-fuzz commands from `fuzz/` so Rust selects its nightly toolchain. `mise exec` supplies the pinned
cargo-fuzz executable even without shell activation. Replace `crash-HASH` below with the actual saved filename (which
may also have a `timeout-` or `oom-` prefix).

```sh
cd fuzz
mise exec -- cargo fuzz run cafebabe_parse_class artifacts/cafebabe_parse_class/crash-HASH -- \
  -timeout=5 -rss_limit_mb=2048
mise exec -- cargo fuzz tmin cafebabe_parse_class artifacts/cafebabe_parse_class/crash-HASH -- \
  -timeout=5 -rss_limit_mb=2048
```

A file argument replays that input rather than starting a mutation session. Minimization tries to find a smaller input
with the same failure. Keep the original artifact and use the minimized path reported by the tool. Resource-limit
failures may be machine-dependent; reproduce with the same limits and assess whether they indicate a real parser issue.

After diagnosis, add a deterministic regression test to the affected crate's `tests/` directory, fix the issue, replay
the saved input, and run both the regression test and another fuzzing session. Do not merely discard the input or
suppress the panic.

Replace the target name and artifact path for findings in another target. For a custom bounded session, after seeding
from the repository root:

```sh
cd fuzz
mise exec -- cargo fuzz run cafebabe_parse_class -- \
  -max_total_time=300 -max_len=65536 -timeout=5 -rss_limit_mb=2048
```
