---
default: patch
---

`prefer-iterable-every` reports filtered-length comparisons only when both sides use the same simple receiver or explicit `this` field, not unrelated collections or repeated calls.
