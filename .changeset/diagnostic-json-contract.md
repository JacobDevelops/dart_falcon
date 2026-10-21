---
default: minor
---

Serialize JSON diagnostic severities as lowercase `error`, `warning`, and `info`. Omit `line` and `col` when a source position has not been resolved instead of emitting zero.

Omit unresolved source positions from text diagnostics as well, rather than printing `0:0`.
