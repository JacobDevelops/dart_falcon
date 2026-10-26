---
default: minor
---

Add checksum-verified self-update and version selection to the Dart pub launcher, cargo-binstall release metadata, optional Minisign signatures and macOS signing and notarisation, and checksum-based Homebrew tap publishing.

Scope saved native-version selections to the Dart launcher package version so upgrading or downgrading the launcher defaults to its matching native release instead of reusing an older selection. Legacy selections are ignored; `FALCON_VERSION` remains an explicit override.
