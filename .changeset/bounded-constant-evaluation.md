---
default: patch
---

Bound constant evaluation memory, share cached constant values across a file's rules, and evaluate reference chains iteratively without a depth limit while preserving the independent constructor nesting limit. Avoid quadratic repeated folding of constant aliases in async-safety rules.
