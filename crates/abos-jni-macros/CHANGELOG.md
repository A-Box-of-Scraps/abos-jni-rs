<!-- Based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). -->

# Changelog

## [Unreleased]

### Fixed

- Emit custom type-mapping checks in stable order, independent of hash iteration and mapping insertion order (#852).

### Changed

- Use local property-name enums in the `bind_java_type!` and `native_method!` parsers while preserving existing syntax
  and diagnostics.
- Split `is_instance_of` code generation into shared type resolution and smaller assignability-check, cast-method, and
  conversion-trait generators without changing generated behavior.

### Added

- Regression tests for property parsing, diagnostics, and `is_instance_of` generation, including custom type mappings,
  JNI crate overrides, and pending-exception guards.

## [0.1.0] - 2026-11-11
