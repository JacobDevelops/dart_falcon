---
default: minor
---

Extract inline string literals into named variables or constants. Invocation exemptions apply only to direct arguments, interpolation expressions are checked, and `allow_only_once` counts exempt literals too.

Count string literal patterns for `allow_only_once` while keeping pattern literals and annotations exempt from diagnostics. Directive URIs and conditional-import configuration strings remain excluded from occurrence counts.
