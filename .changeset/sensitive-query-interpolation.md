---
default: patch
---

Ignore Dart interpolation source punctuation when detecting sensitive URL query parameters, so embedded `#` or `?` characters do not hide static secret-bearing keys and `&token=` inside an expression does not trigger a false warning.
