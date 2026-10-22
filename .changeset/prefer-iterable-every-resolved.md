---
default: patch
---

`prefer-iterable-every` now uses resolved analysis and matches only SDK `Iterable.where` calls, so files without semantic resolution no longer report.
