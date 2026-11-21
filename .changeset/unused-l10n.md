---
default: minor
---

Implement `unused-l10n` to flag template ARB message keys with no identifier references outside generated localization files in analyzed Dart sources, accepting BOM-prefixed templates, warning once and skipping when a template is malformed or unreadable, and refreshing in the editor when ARB files or their directories change.
