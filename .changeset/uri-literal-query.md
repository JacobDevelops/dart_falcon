---
default: patch
---

Detect sensitive keys in literal `query` arguments to SDK `Uri` constructors, including interpolated values, without flagging shadowed constructors or unrelated query calls.
