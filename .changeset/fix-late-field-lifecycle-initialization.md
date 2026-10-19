---
default: patch
---

Allow late State fields to be released in dispose when synchronous initState initializes them through a direct assignment or directly reads a lazy initializer in its initial straight-line expression and declaration statements. Conditional, deferred and uninitialized fields still produce diagnostics, with guidance to initialize them unconditionally or make them nullable.
