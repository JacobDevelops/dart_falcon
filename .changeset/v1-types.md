---
default: minor
---

Add `avoid-collection-equality-checks`, `avoid-unrelated-type-casts`, `avoid-unsafe-collection-methods`, `avoid-uncaught-future-errors` and the first `security` rule, `avoid-weak-cryptographic-algorithms`. All but `avoid-unsafe-collection-methods` are recommended, since it flags every `.first` on a list that might be empty.
