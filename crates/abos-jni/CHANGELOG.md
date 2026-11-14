<!-- Based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). -->

# Changelog

## [Unreleased]

### Fixed

- Preserve NULs, supplementary characters, and unpaired UTF-16 surrogates when borrowing Android Java strings. ART's
  four-byte sequences are normalized before exposing a strict MUTF-8 `JNIStr`; cleanup and `into_raw()` continue to use
  the original JVM buffer.

## [0.1.0] - 2026-11-11
