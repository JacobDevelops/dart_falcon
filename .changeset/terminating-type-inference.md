---
default: patch
---

Falcon now revisits only declarations affected by newly inferred types instead of repeatedly scanning every file. Inference preserves known types and reports an internal error if unresolved dependencies keep changing rather than returning incomplete results. The `avoid-dynamic-calls` rule no longer reports members whose inherited type cannot be resolved.
