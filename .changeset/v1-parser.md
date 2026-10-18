---
default: patch
---

Parse switch-expression guards with a parenthesized operand, like `when (a = b) => 1`, as guards. They were read as closures, which broke the arm and anything checking the guard.
