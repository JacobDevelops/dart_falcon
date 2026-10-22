---
default: patch
---

Fix `prefer-iterable-every` to flag non-negated filtered emptiness and recommend negating its predicate. Negated filtered emptiness is no longer reported, and filtered-length comparisons continue to recommend the original predicate.
