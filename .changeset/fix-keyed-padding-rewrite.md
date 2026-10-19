---
default: patch
---

Exclude Padding wrappers and child Containers with non-null keys from avoid-wrapping-in-padding because moving the padding changes the keyed element's bounds. Literal-null keys still allow the diagnostic.
