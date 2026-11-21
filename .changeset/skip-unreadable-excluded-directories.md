---
default: patch
---

Warn and skip unreadable directories outside every positive `files.includes` pattern, so excluding a directory such as `"!docker/pgdata"` keeps it from failing the run.
