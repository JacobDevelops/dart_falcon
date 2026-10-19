---
default: patch
---

Report nonconstant GlobalKey constructors used directly as widget key arguments, including prefixed constructors and type aliases. Preserve shadowed names, constant keys, value-stable GlobalObjectKeys and cached widgets in field or top-level initializers, while still checking callbacks and local variables.
