---
default: patch
---

Resolve override and deprecation annotations to their `dart:core` identities, including prefixed SDK annotations and one-hop constant aliases of `override` and `deprecated`. User-defined lookalikes no longer suppress override-related diagnostics, exempt unused constructor parameters, or trigger deprecation-message diagnostics, and deprecated-member usage recognizes the same `Deprecated` constructor identities as deprecation consistency checks.
