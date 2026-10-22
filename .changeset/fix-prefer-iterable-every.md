---
default: patch
---

Fix `prefer-iterable-every` to flag non-negated filtered emptiness and recommend negating its predicate. Negated filtered emptiness, including equality with `false` and inequality with `true`, is no longer reported, and filtered-length comparisons continue to recommend the original predicate.
