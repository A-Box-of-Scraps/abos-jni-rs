# Fuzzing

This isolated workspace tests `abos-cafebabe::parse_class` without a JVM. It has its own nightly toolchain and lockfile;
normal workspace builds keep their existing toolchain and dependencies.

## Quick start

From the repository root, with mise and rustup installed:

```sh
mise run setup:fuzz
mise run test:fuzz-seeds
mise run fuzz:build
mise run fuzz:smoke
```

The setup task installs the cargo-fuzz version pinned in `mise.toml` and activates the toolchain pinned in
`fuzz/rust-toolchain.toml`, installing missing components. It does not change your default toolchain. A C++11-capable
compiler is required to build libFuzzer. On GNU/Linux, the repository's linker configuration also requires mold (`mise
install mold`).

`fuzz:smoke` seeds the working corpus, then fuzzes for roughly 60 seconds, excluding compilation. The initial limits are
64 KiB generated inputs, five seconds per input, and a 2048 MiB process RSS limit. These are operating limits, not
class-file format guarantees; larger inputs and tighter or looser resource budgets can be tested separately.

For an ongoing local session:

```sh
mise run fuzz:cafebabe
```

Stop with Ctrl-C. Interesting inputs are saved as the session runs; later sessions reuse them. Fuzzing tasks are
deliberately separate from `mise run test` and ordinary workspace checks. The root formatting tasks include the fuzz
workspace.

## What the target checks

The harness passes arbitrary bytes to the parser. Returning `Err` is expected for invalid or unsupported inputs. Panics,
crashes, sanitizer reports, and resource-limit failures need investigation; the harness neither unwraps the parse result
nor catches panics. Cargo-fuzz's default AddressSanitizer configuration is used.

LibFuzzer mutates inputs and keeps examples that exercise interesting execution paths. This is coverage-guided testing:
seed files help it reach beyond the class-file header rather than repeatedly rejecting random bytes at the magic number.

The target checks robustness, not complete metadata correctness or JVM bytecode validity. A clean run is not proof that
there are no bugs. Deterministic regression tests remain necessary.

## Seeds and generated files

The three committed files in `seeds/cafebabe_parse_class/` are hand-assembled, Java 8-version (major 52) class-file
metadata fixtures, not copied from a JDK or third-party archive:

| File            | Contents                                                                                                                                                             |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `minimal.class` | `java/lang/Object`, no superclass or members                                                                                                                         |
| `members.class` | A class implementing `Runnable`, a static field, a native method, and opaque attributes on the class and members                                                     |
| `unicode.class` | An accented class name, a supplementary-character field name encoded as a modified UTF-8 surrogate pair, and an unused string constant containing modified UTF-8 NUL |

They are parser fixtures, not JVM-loadability tests. `tests/seeds.rs` checks their expected extracted metadata without a
JDK. When editing these fixtures, update the assertions and run `mise run test:fuzz-seeds`.

`mise run fuzz:seed` copies missing seed files into `corpus/cafebabe_parse_class/`. It does not replace existing files
or remove discoveries. If a committed seed changes, remove only its corresponding named copy from the working corpus
before seeding again.

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

After diagnosis, add a deterministic regression test to `crates/abos-cafebabe/tests/`, fix the parser, replay the saved
input, and run both the regression test and another fuzzing session. Do not merely discard the input or suppress the
panic.

For a custom bounded session, after seeding from the repository root:

```sh
cd fuzz
mise exec -- cargo fuzz run cafebabe_parse_class -- \
  -max_total_time=300 -max_len=65536 -timeout=5 -rss_limit_mb=2048
```
