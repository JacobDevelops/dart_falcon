---
default: patch
---

Resolve override and deprecation annotations consistently to their `dart:core` identities. Prefixed SDK annotations are recognized, while user-defined lookalikes no longer suppress override-related diagnostics, exempt unused constructor parameters, or trigger deprecation-message diagnostics. Deprecated-member usage also recognizes the same resolved `Deprecated` constructor identities as deprecation consistency checks.

`avoid-returning-widgets`, `avoid-unused-parameters`, `provide-deprecation-message`, `boolean-prefixes`, `member-ordering`, and `avoid-unused-constructor-parameters` now use resolved rather than file analysis. They require semantic resolution and do not report diagnostics for files without semantic resolution.

`exhaustive-cases` also uses resolved deprecation identities when choosing between static constants with the same value, so a user-defined `Deprecated` annotation no longer changes the suggested case.

Recognize one-hop constant aliases of SDK `override` and `deprecated` annotations.
