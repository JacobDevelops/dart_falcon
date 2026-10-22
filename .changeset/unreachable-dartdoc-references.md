---
default: patch
---

`unreachable-from-main` now uses the shared Dartdoc reference parser: Markdown link display text and link definitions, inline and block code (fenced or indented), Dartdoc directives, and trailing inline line-documentation comments no longer keep declarations reachable, while reference-style destination labels still count as references.
