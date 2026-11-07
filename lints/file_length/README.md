# File length

`file_length` warns when a physical Rust source file exceeds 600 lines.

## Rules

- All lines count, including comments and blank lines. An unterminated final line counts too.
- Empty files and files with exactly 600 lines are allowed.
- Each file is reported once per compiler invocation, even if multiple modules load it. Inline modules do not get a
  separate line budget.
- Only local `.rs` files loaded by the compiler are checked. Dependency sources, virtual sources, hidden paths,
  `target/` directories, and Cargo's `OUT_DIR` are skipped.

Use `#![allow(file_length)]` at the crate or external module level, or `#[allow(file_length)]` on an external module
declaration, for exceptions. Files loaded through `include!` and other files without a module boundary use the crate's
lint level.

This is a compiler lint, not a repository-wide scanner. It cannot check unreferenced files, external modules disabled by
configuration, or packages outside the Cargo invocation. Run it for the relevant targets, features, and packages for
broader coverage. Git ignore rules do not control the lint; use lint attributes for exceptions outside the skipped
paths.

## Development

This crate belongs to the shared `lints/` workspace, which owns the pinned nightly toolchain, Dylint dependency
versions, and lockfile.

```sh
mise run test:lints
mise run lint
```

The root lint task loads all lint libraries and treats warnings as errors. UI fixtures under `ui/` are padded in a
temporary directory by `tests/unit/mod.rs`, preserving small, readable checked-in files. `.stderr` snapshots record the
expected diagnostics. Tests cover the boundary, unterminated lines, module and included files, duplicate loading, lint
attributes, and generated or hidden files.
