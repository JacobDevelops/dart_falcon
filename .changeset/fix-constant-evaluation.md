---
default: patch
---

Evaluate constant maps in source order and integers with exact 64-bit semantics, including large integer literals in `double` contexts, so `avoid-redundant-argument-values` compares constants the way the analyzer does.
