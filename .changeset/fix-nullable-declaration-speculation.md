---
default: patch
---

Avoid exponential parsing time for nested nullable-declaration and conditional-expression statement heads, while preserving nullable declarations with conditional initializers. Ensure statement recovery makes progress when nested closures reach the parser depth ceiling.
