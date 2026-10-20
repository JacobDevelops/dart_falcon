---
default: patch
---

Fix `unnecessary-nullable` false positives for method tear-offs, cascade references, and parameter writes that may be null. Resolve cross-file rules consistently even when no file rules require semantic resolution.
