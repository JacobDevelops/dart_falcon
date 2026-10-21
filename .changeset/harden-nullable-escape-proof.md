---
default: patch
---

Fix `unnecessary-nullable` false positives for method tear-offs, cascade references, parameter writes that may be null (including compound assignments and `++`/`--`), and arguments typed `FutureOr<T?>` or as an extension type that doesn't implement `Object`. Resolve cross-file rules consistently even when no file rules require semantic resolution.
