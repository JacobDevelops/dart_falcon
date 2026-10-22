---
default: patch
---

`falcon migrate` refuses ambiguous legacy exclusions and repairs that would erase a whole section. Unusable or unsupported override scopes discard the whole override with its original index, while invalid top-level file scopes fail without writing.
