---
default: patch
---

Avoid repeated path canonicalization for CLI inputs that are already indexed, while preserving symlink and parent-component resolution.
