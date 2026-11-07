# Release Issues

Issues are ordered by priority. The two JNI safety issues must be fixed before release. Each fix should include a
regression test, followed by broader adversarial safe-API testing and source-versus-bytecode parity coverage.

## 1. P1: `new_object_array` allows incompatible initial values

**Location:** `crates/abos-jni/src/api/env/arrays.rs:54–66`

The function passes the initializer to JNI without checking whether it belongs to the requested element class.

**Reproduced:** creating a `String[]` initialized with a plain `Object` succeeds. Safely casting it to
`JObjectArray<JString>` and reading an element produces a `JString` wrapping that plain object. Calling
`try_to_string` aborts under `-Xcheck:jni` with “JNI string operation received a non-string”.

**Fix:** evaluate the initializer's `AsRef` once and validate non-null values against the resolved element class before
allocation. Test the subsequent typed cast as well as array creation.

## 2. P1: Single-letter class names can make primitive arrays pass object-array casts

**Location:** `crates/abos-jni/src/api/objects/jobject_array.rs:360–368`

`class_name()` assumes every one-character element name is a primitive descriptor.

**Reproduced:** for a default-package Java class named `I`, `JObjectArray<I>::class_name()` returns `[I` instead of
`[LI;`. Safely casting an `int[]` to `JObjectArray<I>` succeeds. Reading an element aborts under `-Xcheck:jni` with
“Object array expected but not received for JNI array operation”.

**Fix:** distinguish array descriptors from object binary names without the one-character heuristic. Test default-package
names that overlap primitive descriptor letters.

## 3. P1: Generic Java source produces nonexistent JNI signatures

**Location:** `crates/abos-jbindgen/java/com/jbindgen/Parser.java:494–497,554–555`

Type variables retain names such as `T` instead of their erased JVM types. The descriptor helper also incorrectly assumes
every type variable erases to `Object`.

**Reproduced:** for `Generic<T extends Number>` with `public T echo(T value)`, source generation emits `.T` arguments and
returns. Binding initialization fails with `MethodNotFound { name: "echo", sig: "(LT;)LT;" }`. Bytecode generation
correctly uses `java.lang.Number`.

**Fix:** apply `Types.erasure` consistently when constructing runtime type metadata, including fields and arrays. Test
source/bytecode signature parity.

## 4. P2: Non-static inner-class constructors omit the enclosing instance

**Location:** `crates/abos-jbindgen/java/com/jbindgen/Parser.java:429–460`

Constructor parameters are copied directly from the source model, missing the enclosing-instance parameter present in
bytecode.

**Reproduced:** `Outer.Inner(int)` generates `(I)V`, while `javap` reports `(Laudit/Outer;I)V`. Binding initialization
fails with `MethodNotFound`, preventing ordinary methods on that wrapper from working too.

**Fix:** include the enclosing-instance parameter for non-static member classes. Explicitly reject unsupported
captured/local-class cases rather than generating incorrect signatures.

## 5. P2: Records and annotation types receive invalid class names

**Location:** `crates/abos-jbindgen/java/com/jbindgen/Parser.java:685–695`

The handwritten binary-name builder recognizes only classes, interfaces, and enums.

**Reproduced:** both `audit.Point`, a record, and `audit.Marker`, an annotation type, generate
`pub java => "audit.",` instead of their actual class names.

**Fix:** use `Elements.getBinaryName` instead of reconstructing binary names manually. Include nested records and
annotations in tests.

## 6. P2: Source generation silently omits enum constants

**Location:** `crates/abos-jbindgen/java/com/jbindgen/Parser.java:331–332`

Field collection handles `ElementKind.FIELD`, but not `ENUM_CONSTANT`.

**Reproduced:** source generation for `enum Color { RED, BLUE }` exposes neither constant. Bytecode generation correctly
emits both accessors.

**Fix:** process enum constants as fields and test source/bytecode API parity.

## Verification baseline

- Workspace tests with all features: 633 passed, 0 failed, 7 ignored.
- Strict workspace Clippy: passed.
- Default-feature `abos-jni` tests: passed.
- Explicit JVM invocation test: passed.
- Both safety failures reproduced through safe public APIs in isolated processes with `-Xcheck:jni`.

The audit was performed on Linux with JDK 25. It was not an exhaustive soundness review or Windows/Android validation.
Source locations refer to the audited revision and may shift as fixes are applied.

## Adversarial safe-API testing gap

The passing suite did not catch the two cases where safe public calls produce invalid typed references. Regression tests
for those cases are necessary, but broader coverage should exercise combinations of safe operations, not just individual
methods with valid inputs.

**Coverage to add:**

- Incompatible array initializers, method arguments, and field values; distinguish accepted nulls from invalid references.
- Primitive arrays, object arrays, nested arrays, and casts between incompatible element types.
- Default-package and single-letter class names, including names that overlap primitive descriptors.
- Safe `AsRef` implementations that return different valid references on successive calls, to catch validation/use
  mismatches.
- Pending Java exceptions, invalid indexes, and boundary lengths, checking that failures leave the environment usable
  after the exception is handled.
- Compile-fail tests for local references escaping their frame or crossing threads.

**Test approach:** keep runtime reproductions entirely within safe public Rust APIs. Run potentially fatal JNI cases in
isolated child processes with `-Xcheck:jni`, and have the parent assert successful completion and the expected result.
Exercise returned references through subsequent casts, reads, and calls so invalid wrappers cannot pass unnoticed.

**Acceptance:** invalid inputs are rejected through the documented error or exception behavior, without invalid typed
references or JVM aborts. Valid edge cases continue to work.

## Source-versus-bytecode parity suites

The source-generation failures show that generated text alone is not enough to validate bindings. Both input paths should
agree on the runtime API represented by the same compiled Java fixture.

**Coverage to add:**

- Unbounded and bounded generics, generic methods, and arrays of type variables.
- Static nested classes and non-static inner classes, including their constructor descriptors.
- Records, annotation types, enums and enum constants, including nested declarations.
- Overloads, inheritance, covariant returns, visibility, and static/final members.
- Default-package classes and unusual but valid Java identifiers.

**Test approach:** compile each fixture once, then generate bindings independently from its source and resulting
class files or JAR. Compare normalized class names and member signatures, including descriptors and static flags, rather
than requiring identical Rust text. Parameter names, documentation, source-only annotations, and compiler-generated
synthetic/bridge members need explicit comparison rules; differences must not be silently ignored.

Compile both generated binding sets in separate Rust modules or test crates, initialize them against the same compiled
Java classes, and exercise representative constructors, methods, and field accessors. Assert expected member presence so
an omission shared by both paths cannot make a test pass.

**Acceptance:** supported fixtures expose the expected runtime API through both paths. Unsupported constructs return
clear generation errors rather than successful output with invalid names, missing members, or nonexistent signatures.
