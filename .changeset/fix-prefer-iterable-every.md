---
default: patch
---

Fix `prefer-iterable-every` to flag non-negated filtered emptiness and recommend negating its predicate. Negated filtered emptiness, including equality with `false` and inequality with `true`, is no longer reported, and filtered-length comparisons continue to recommend the original predicate.

Move `prefer-iterable-every` from file to resolved analysis and match only SDK `Iterable.where` calls; files without semantic resolution no longer report. Filtered-length comparisons require the same simple receiver or explicit `this` field, not unrelated collections or repeated calls.
