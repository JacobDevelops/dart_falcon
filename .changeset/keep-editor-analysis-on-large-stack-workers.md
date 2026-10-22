---
default: patch
---

Prevent the language server from crashing when a dependency exports many files on machines with few available processors. Dependency loading and analysis now stay on the shared workers with enough stack space.
