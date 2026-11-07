# Tests outside src

`tests_outside_src` requires test implementations outside each package's `src/` directory. Use a path-included child
module to retain access to private members:

```rust
#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;
```

Any location outside `src/` is allowed. Under `tests/`, use a subdirectory to avoid compiling the module as a standalone
integration test.

## Rules

Inside a package's `src/`, the lint rejects:

- `#[test]` and qualified attributes ending in `::test`.
- Test-only code gated by `#[cfg(test)]` or compound predicates such as `#[cfg(all(test, windows))]`.
- Test attributes introduced through `#[cfg_attr(...)]`.

Out-of-line module declarations with a direct `#[path]` resolving outside `src/` are allowed. The nearest ancestor
`Cargo.toml` defines the package boundary; module names do not matter.

Documentation examples, `#[cfg(not(test))]`, code shared with production through `#[cfg(any(test, ...))]`, and files
outside `src/` are not rejected.

The lint runs before macro expansion. It checks inactive branches in parsed files, but not unloaded files or
macro-generated tests. Run with the supported targets and features for broader coverage.

## Checks

Run from the repository root:

```sh
mise run lint
mise run test:lints
```
