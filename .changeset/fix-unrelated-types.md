---
default: patch
---

Stop `avoid-unrelated-type-casts` and `avoid-unrelated-type-assertions` reporting types that can share a runtime value, such as unrelated interfaces, generic enums, `FutureOr` and subtypes of sealed classes.
