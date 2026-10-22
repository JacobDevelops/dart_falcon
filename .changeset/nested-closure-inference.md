---
default: patch
---

Make checks of deeply nested closures finish promptly without repeatedly analyzing the same closure bodies. Fix closure return types to match Dart when a call that never returns uses a block-scoped variable or a promoted callable that is later assigned a different type.
