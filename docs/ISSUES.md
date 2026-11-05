# Deferred dependency issues

Track dependency bugs and temporary workarounds here. Keep resolved entries with their status updated so the reason for
removing a workaround remains discoverable. Active workspace fixes are tracked in [TODO.md](../TODO.md).

## JNI-MACROS-001: Explicit Java member names can panic during macro expansion

- **Status:** Open; deferred until the JNI macro crate is forked into this workspace.
- **Affected dependency:** `jni-macros` 0.22.4, used through `jni::bind_java_type!`.
- **Planned fix location:** `crates/abos-jni-macros/` (not yet present).
- **Upstream source:** `crates/jni-macros/src/bind_java_type.rs`, function `parse_name_spec`.

### Symptom and reproduction

Java permits `$` in method and field names. JNI lookup can use those names, but generating bindings for such members
currently causes the downstream macro to panic, even when the Rust identifier is valid:

```rust
jni::bind_java_type! {
    pub Example => "example.Example",
    fields {
        dollar_value {
            name = "$value",
            sig = jint,
            get = dollar_value,
            set = set_dollar_value,
        },
    },
    methods {
        fn foo_bar {
            name = "foo$bar",
            sig = () -> jint,
        },
    },
}
```

The panic reports that the Java name is not a valid identifier. Other Java member names that are not Rust identifiers
can encounter the same problem; this is not limited to `$`.

### Cause

When parsing an explicit Java name string, `parse_name_spec` unnecessarily constructs a Rust identifier:

```rust
let java_name = lit.value();
let rust_name = format_ident!("{}", java_name);
Ok((java_name, rust_name))
```

Both the method and field parsers discard the returned identifier:

```rust
let (jname, _) = parse_name_spec(&body_content)?;
```

The macro already has the separate Rust member name. Converting the Java lookup string into a Rust identifier is
unnecessary and introduces the panic.

### Current workaround

Introduced in commit `b40faa469110d70c3b1e6edaa900168f6a7fb67d` (`fix(jbindgen): normalize generated Rust identifiers`).
This commit also contains permanent identifier and collision fixes; remove only the dependency-specific guard and
related limitation handling after migration, not the entire commit.

`crates/abos-jbindgen/src/internal/generator/identifiers.rs::validate_java_member_name` rejects names that would trigger
this panic. Callers can explicitly omit a member with `Builder::skip_signature`; renaming its Rust binding alone does
not avoid the dependency bug.

Coverage lives in:

- `crates/abos-jbindgen/tests/java/UnsupportedNames.java`
- `crates/abos-jbindgen/tests/identifiers.rs`
- Unit tests in `crates/abos-jbindgen/src/internal/generator/identifiers.rs`

### Fix checklist for the fork

- [ ] Audit callers of `parse_name_spec` in the imported macro source.
- [ ] Retain explicit Java names as strings without constructing a Rust identifier from them. Keep validation of the
      separately supplied Rust identifiers and preserve the identifier-form `name = identifier` syntax.
- [ ] Add macro compilation tests for explicit Java names containing `$` in fields, methods, and native methods.
- [ ] Add JVM execution tests that access a `$`-named field and invoke a `$`-named method through valid Rust names.
- [ ] Switch `abos-jbindgen` to the fixed fork, then remove `validate_java_member_name` and its call sites. Remove the
      direct `unicode-ident` dependency if it is no longer used elsewhere.
- [ ] Convert rejection tests and `UnsupportedNames.java` into successful generation and execution regressions. Update
      the public limitation documentation in `crates/abos-jbindgen/src/lib.rs` and the note in `TODO.md`.
- [ ] Mark this issue resolved and record the fixing commit.
