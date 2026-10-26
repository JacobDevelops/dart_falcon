---
default: patch
---

Fix `falcon check` hanging on expressions nested past the parser's depth limit, such as thousands of nested list literals; the excess nesting now reports one syntax error.
