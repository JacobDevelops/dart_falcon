---
default: patch
---

Keep binary-chain analysis linear when operands write local variables, and avoid repeatedly scanning expression prefixes in `use-existing-variable`. Carry each evaluated left operand's type into the next operator and its right-hand context, matching Dart's left-to-right evaluation even when a later operand changes that local's promoted type. The rule also evaluates declarations in order, fixing missed reuse diagnostics when a computation depends on an earlier variable in the same declaration statement.
