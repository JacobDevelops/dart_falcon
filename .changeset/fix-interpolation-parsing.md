---
default: patch
---

Fix string interpolation parsing. `'${await x}'` inside an async function was dropped from the syntax tree, so anything used only there looked unused (for example to `avoid-unused-parameters`). An escaped brace such as `${a\}b}` is now rejected with a parse error, matching Dart.
