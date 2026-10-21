---
default: patch
---

Serialize JSON diagnostic severities as lowercase `error`, `warning`, and `info`. Omit `line` and `col` when a source position has not been resolved instead of emitting zero.
