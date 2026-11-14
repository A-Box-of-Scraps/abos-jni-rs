<!-- Based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). -->

# Changelog

## [Unreleased]

### Fixed

- Check `JIterator::has_next` before advancing for compatibility with Android iterators that return invalid entries on
  exhaustion, while preserving pending exceptions and the `next` exception fallback. Add Android `ArrayMap` coverage and
  iterator benchmarks that include the additional JNI call.

- Document the native registration code-lifetime contract, descriptor lifetime limits, and the need to coordinate
  in-flight calls before unloading a library.

- Make version-sensitive `JMap` methods optional at runtime; missing methods return `Error::MemberUnavailable` without
  preventing basic map operations from initializing.

- Avoid invoking loggers during automatic TLS/FLS thread detachment, when logger TLS may already be destroyed.

- Preserve NULs, supplementary characters, and unpaired UTF-16 surrogates when borrowing Android Java strings. ART's
  four-byte sequences are normalized before exposing a strict MUTF-8 `JNIStr`; cleanup and `into_raw()` continue to use
  the original JVM buffer.

## [0.1.0] - 2026-11-11
