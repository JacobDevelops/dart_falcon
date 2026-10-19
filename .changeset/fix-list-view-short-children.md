---
default: patch
---

Only suggest ListView for a scrollable Column when its children list contains at least two unconditional children. Skip absent, empty, single-child and dynamic lists, and do not count spread or control-flow elements toward that minimum. Preserve the existing alignment and padding advice without offering an automatic fix.
