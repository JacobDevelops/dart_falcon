---
default: minor
---

Add an optional rule that flags getters exposing only a private final field. Upcasting getters remain allowed by default; set `ignore_upcast: false` to report them.