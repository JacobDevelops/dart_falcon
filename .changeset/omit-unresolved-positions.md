---
default: minor
---

Omit unresolved source positions from diagnostics: JSON output leaves out `line` and `col` instead of emitting zero, and text output no longer prints `0:0`.
