# Explicit binding types

`explicit_binding_types` requires type annotations on simple `let` and `let mut` bindings whose initializer does not
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

- Literals, casts, and explicit type ascriptions need no annotation. References, negation, tuples, and arrays follow the
  same rule when their contents show the type.
- Struct literals and `Type::new()` need no annotation when the named type matches the resulting struct or enum and
  required type and const arguments are explicit. `Vec::new()` and `Vec::<_>::new()` require annotations.
- Other calls, field accesses, and variable copies require annotations. Type aliases and constructors other than `new`
  are not recognized as showing the type.
- Type annotations containing `_`, including nested placeholders such as `Vec<Option<_>>`, are insufficient.
- Destructuring, discarded values, generated bindings, closures, and opaque `impl Trait` results are skipped. `if let`,
  `while let`, match arms, and loop patterns are not checked.

Use `#[allow(explicit_binding_types)]` for exceptions. Diagnostics offer guidance, not automatic fixes.

## Checks

Run from the repository root:

```sh
mise run lint
mise run test:lints
```
