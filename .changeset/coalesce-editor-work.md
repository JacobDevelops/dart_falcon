---
default: patch
---

Keep the language server responsive during bursts of edits, saves and file changes by coalescing queued work and dropping outdated read-only requests.
