---
default: minor
---

Remove `prefer-any-or-every`, which reported the same `where(predicate).isEmpty` calls as `prefer-iterable-every`. Existing `prefer-any-or-every` config keys and suppressions are reported with a pointer to `complexity/prefer-iterable-every`.
