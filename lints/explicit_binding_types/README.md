# Explicit binding types

`explicit_binding_types` requires a type annotation on simple `let` and `let mut` bindings when the initializer does not
show the type.

```rust
let player: ServerPlayer = get_player();
let other: ServerPlayer = server.get_player();

// No annotation needed:
let name = "toto";
let enabled = true;
let player = ServerPlayer::new();
let players = Vec::<ServerPlayer>::new();
```

## Rules

- Literals, casts, and explicit type ascriptions are obvious. References, negation, tuples, and arrays are obvious when
  their contents are obvious.
- Struct literals and `Type::new()` are obvious when the named type matches the resulting struct or enum, and its
  required type and const arguments are written explicitly. `Vec::new()` and `Vec::<_>::new()` need an annotation.
- Other function calls, method calls, field accesses, and variable copies need an annotation. A function named `new`
  that returns a different type is not treated as a constructor.
- `_` placeholders, including nested ones such as `Vec<Option<_>>`, do not count as explicit types.
- Destructuring, discarded values (`let _ = ...`), generated bindings, closures, and opaque `impl Trait` results are
  skipped. This rule does not cover `if let`, `while let`, match arms, or loop patterns.
- Type aliases and constructors other than `new` are not recognized as obvious yet.

Use `#[allow(explicit_binding_types)]` on an exceptional binding when spelling out its type would be impractical.
Diagnostics provide help rather than automatic fixes because compiler-rendered types are not always valid source
annotations.

## Development

This crate belongs to the shared `lints/` workspace, which owns the pinned nightly toolchain, `rustc-dev`, dependency
versions, and lockfile. The application workspace continues to use its own toolchain. Dylint's compiler APIs are
unstable; update the toolchain and lint implementation together.

From the repository root:

```sh
mise install
mise run lint
mise run test:lints
```

`lint` runs Clippy followed by Dylint, treating warnings as errors. `test:lints` compares diagnostics from `ui/main.rs`
with `ui/main.stderr`, covering both accepted and rejected bindings. Review snapshot changes when changing the rule. The
root formatting tasks also format this lint crate; UI fixtures are maintained separately to preserve diagnostic
locations.
