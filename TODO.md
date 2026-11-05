# Rust correctness fixes

- [x] **1. Track Java build inputs by default.** Enable source-change directives in `abos-javac`, preserve an explicit
      opt-out, and test Cargo rebuilds after Java source edits and source-directory additions.
- [x] **2. Normalize generated Rust identifiers consistently.** Handle keywords (including `gen`), invalid identifier
      characters, and parameter-name collisions. Preserve the original Java names for JNI lookup and compile regression
      fixtures. Unsupported Java member names (such as names containing `$`) now fail early with an explicit skip
      workaround because the downstream JNI macro cannot represent them. The fork fix is tracked as
      [JNI-MACROS-001](docs/ISSUES.md#jni-macros-001-explicit-java-member-names-can-panic-during-macro-expansion).
- [x] **3. Share constructor and member name allocation.** Reserve constructor names alongside methods and field
      accessors. Use full member signatures to distinguish overloads and test constructor/method collisions.
- [ ] **4. Handle JAR metadata and multi-release entries.** Exclude metadata classes such as `module-info` and define a
      deterministic policy for selecting one class version before generating bindings. Compile modular and multi-release
      fixtures.
- [ ] **5. Reject incomplete Android generation by default.** Propagate parsing and generation failures rather than
      silently dropping classes. If partial output is supported, require an explicit option and return structured
      diagnostics.
