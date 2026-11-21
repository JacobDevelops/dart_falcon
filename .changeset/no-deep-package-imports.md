---
default: minor
---

Add `no-deep-package-imports` to flag imports of dependency libraries already exported unrestricted by their public barrel, warning once and skipping the check when a dependency's exports can't be read.
