---
default: patch
---

Resolve override and deprecation annotations consistently to their `dart:core` identities. Prefixed SDK annotations are recognized, while user-defined lookalikes no longer suppress override-related diagnostics, exempt unused constructor parameters, or trigger deprecation-message diagnostics. Deprecated-member usage also recognizes the same resolved `Deprecated` constructor identities as deprecation consistency checks.
