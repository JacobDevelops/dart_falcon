---
default: patch
---

Do not treat parameter defaults, relational-pattern operands, or map-pattern keys as implicit constant contexts when suggesting named constants or removing redundant arguments. These positions require constant expressions but still require explicit `const` on constructor calls and collection literals.
