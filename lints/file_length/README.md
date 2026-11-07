# File length

`file_length` warns when a physical Rust source file exceeds 600 lines.

## Rules

- All lines count, including comments, blank lines, and an unterminated final line.
- Each file is reported once per compiler invocation. Inline modules share the file's line budget.
- Only local `.rs` files loaded by the compiler are checked. Dependency sources, virtual sources, hidden paths,
  `target/` directories, and Cargo's `OUT_DIR` are skipped. Git ignore rules do not affect the lint.

Use `#![allow(file_length)]` at the crate or external module level, or `#[allow(file_length)]` on an external module
declaration, for exceptions. Files loaded through `include!` or without a module boundary use the crate's lint level.

Unreferenced files, modules disabled by configuration, and packages outside the Cargo invocation are not checked. Run
with the relevant targets, features, and packages for broader coverage.

## Checks

Run from the repository root:

```sh
mise run lint
mise run test:lints
```
