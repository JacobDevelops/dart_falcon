---
default: patch
---

`exhaustive-cases` uses resolved deprecation identities when choosing between static constants with the same value, so a user-defined `Deprecated` annotation no longer changes the suggested case.
