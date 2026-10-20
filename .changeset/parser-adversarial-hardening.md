---
default: patch
---

Prevent exponential parsing of nested collection literals and malformed lambdas, and recover from deeply nested patterns and types without crashing. Preserve valid long expression and pattern chains while making their default syntax traversal stack-safe. Parse top-level metadata once and ensure malformed switch arms cannot stall the parser.
