<!-- Based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). -->

# Changelog

## [Unreleased]

### Added

- Add JVM-independent `catch_callback` and `catch_callback_with` panic boundaries with explicit callback-specific
  fallback values, protected panic handlers, and documented restricted-context and panic-payload lifetime limits.
  Migrate event/profiling agent examples and heap visitors, with regression coverage for nested panics, hostile payload
  destructors, handler capture cleanup, and native callback return conventions.

## [0.1.0] - 2026-11-11
