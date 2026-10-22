---
default: patch
---

Keep unknown generic arity unknown instead of inventing zero type parameters for types such as `ValueNotifier` and `Provider`, while retaining explicitly curated zero-arity types.
