# Dependency issues

Track dependency bugs and temporary workarounds here. Keep resolved entries with their status updated so the reason for
removing a workaround remains discoverable.

## JNI-MACROS-001: Explicit Java member names can panic during macro expansion

- **Status:** Resolved in the workspace fork.
- **Affected dependency:** `jni-macros` 0.22.4, used through `jni::bind_java_type!`.
- **Fix location:** `crates/abos-jni-macros/src/internal/bind_java_type/parse_method.rs`.
- **Fixing commit:** `fix(jni-macros): preserve explicit Java member names`.
- **Upstream source:** `crates/jni-macros/src/bind_java_type.rs`, function `parse_name_spec`.

### Symptom and reproduction

Java permits `$` in method and field names. JNI lookup can use those names, but generating bindings for such members
caused the upstream macro to panic, even when the Rust identifier is valid:

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
could encounter the same problem; this is not limited to `$`.

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

### Removed workaround

Introduced in commit `b40faa469110d70c3b1e6edaa900168f6a7fb67d` (`fix(jbindgen): normalize generated Rust identifiers`).
This commit also contains permanent identifier and collision fixes; remove only the dependency-specific guard and
related limitation handling after migration, not the entire commit.

`crates/abos-jbindgen/src/internal/generator/identifiers.rs::validate_java_member_name` rejected names that would
trigger this panic. Callers could explicitly omit a member with `Builder::skip_signature`; renaming its Rust binding
alone did not avoid the dependency bug. The guard and direct `unicode-ident` dependency have now been removed.

Regression coverage lives in:

- `crates/abos-jbindgen/tests/java/UnsupportedNames.java`
- `crates/abos-jbindgen/tests/identifiers.rs`
- `crates/abos-jbindgen/tests/runtime/` (JVM field reads/writes and method calls)
- `crates/abos-jni/tests/ui/bind_java_type/pass/explicit_java_names.rs` (fields, methods, native methods, non-Rust
  Unicode names, and identifier-form names)

### Fork resolution

`parse_name_spec` now returns only the Java name string. Its two callers, field parsing and shared method parsing
(including native methods), retain their independently parsed Rust identifiers. Identifier-form names still parse as
Rust identifiers. `abos-jbindgen` already uses the workspace `abos-jni` fork.

### Fix checklist for the fork

- [x] Audit callers of `parse_name_spec` in the imported macro source.
- [x] Retain explicit Java names as strings without constructing a Rust identifier from them. Keep validation of the
      separately supplied Rust identifiers and preserve the identifier-form `name = identifier` syntax.
- [x] Add macro compilation tests for explicit Java names containing `$` in fields, methods, and native methods.
- [x] Add JVM execution tests that access a `$`-named field and invoke a `$`-named method through valid Rust names.
- [x] Switch `abos-jbindgen` to the fixed fork, then remove `validate_java_member_name` and its call sites. Remove the
      direct `unicode-ident` dependency if it is no longer used elsewhere.
- [x] Convert rejection tests and `UnsupportedNames.java` into successful generation and execution regressions. Update
      the public limitation documentation in `crates/abos-jbindgen/src/lib.rs`.
- [x] Mark this issue resolved.
- [x] Record the fixing commit by its title above.
