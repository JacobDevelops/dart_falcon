---
default: minor
---

Add `prefer-early-return`, `prefer-returning-condition`, `prefer-switch-expression`, `prefer-for-in` and `prefer-named-boolean-parameters`. `prefer-named-boolean-parameters` covers the private functions and closures that `avoid-positional-boolean-parameters` skips, so enabling both covers every positional `bool`.
