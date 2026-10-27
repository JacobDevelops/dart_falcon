---
default: minor
---

Add checksum-verified self-update and version selection to the Dart pub launcher, cargo-binstall release metadata, Minisign signatures, macOS signing and notarisation, and checksum-based Homebrew tap publishing.

Scope saved native-version selections to the Dart launcher package version so upgrading or downgrading the launcher defaults to its matching native release instead of reusing an older selection. Legacy selections are ignored; `FALCON_VERSION` remains an explicit override.

Clarify that the pub launcher's same-release SHA256 manifest check provides integrity, not publisher authenticity, and document manual Minisign verification with a trusted public key.
