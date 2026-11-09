# Fuzzing

This isolated workspace fuzzes class-file parsing, binding metadata conversion, JNI descriptors, and string encoding. It
has its own nightly toolchain and lockfile; normal workspace builds keep their existing toolchain and dependencies. The
targets do not start a JVM. Building the workspace requires a JDK because jbindgen embeds a compiled Java parser, even
though the fuzz targets only exercise its Rust bytecode parsers.

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

`fuzz:build` builds all six targets as an optional cache warm-up. Each smoke task waits for the seed task, builds only
its selected target through `cargo fuzz run`, then fuzzes for roughly 60 seconds, excluding compilation. Previously
built targets reuse the build cache. The initial limits per process are 64 KiB generated inputs, five seconds per input,
and a 2048 MiB RSS limit. These are operating limits, not format guarantees.

`fuzz:smoke:all` schedules the six independent smoke tasks concurrently up to mise's job limit. `--jobs 2` runs at most
two at once (roughly three minutes of fuzzing); `--jobs 6` can run all six at once (roughly one minute). Memory limits
apply separately to each process: six workers can consume approximately 12 GiB in total, plus build and system overhead.
Use `--jobs 1` for sequential execution. A failure makes the task fail and, by default, stops the other running tasks.
There are no additional libFuzzer workers inside these tasks.

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

| Target                 | Smoke task suffix      | Checks                                                                                                                                                         |
| ---------------------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cafebabe_parse_class` | `cafebabe`             | Arbitrary class-file bytes must not crash the metadata parser                                                                                                  |
| `jbindgen_parse_class` | `jbindgen-parse-class` | Class metadata, member descriptors, and binding metadata conversion must not crash                                                                             |
| `jbindgen_descriptors` | `jbindgen-descriptors` | `parse_type_descriptor` and `parse_method_descriptor` must handle arbitrary UTF-8 inputs without crashing                                                      |
| `jni_signatures`       | `jni-signatures`       | `JavaType::from_str`, `RuntimeFieldSignature::from_str`, and `RuntimeMethodSignature::from_str`; accepted field/method signatures preserve their original text |
| `jni_string_roundtrip` | `jni-string-roundtrip` | `JNIString::new` output validates through `JNIStr::from_cstr`, and both views decode to the original Rust string                                               |
| `jni_mutf8`            | `jni-mutf8`            | Arbitrary bytes go through safe `CString` construction and `JNIStr::from_cstr`; accepted strings retain their bytes and can be decoded without crashing        |

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
