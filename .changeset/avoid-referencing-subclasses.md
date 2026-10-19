---
default: minor
---

Keeps base classes independent from transitive `extends`/`with` subclasses, excluding implementers and mixin constraints. Exempts `freezed`, `Freezed`, and `unfreezed` annotations by default; `ignored_annotations` replaces this exact-match list.
