---
default: patch
---

Falcon now keeps cached type results tied to their source values, preventing stale results when analysis reuses memory after discarding temporary values.
