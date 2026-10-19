---
default: patch
---

Clarify that replacing a scrollable Column with ListView requires preserving child alignment and padding explicitly, including ListView's default MediaQuery padding. Keep this as advice without an automatic fix, rather than promising a layout-neutral replacement.
