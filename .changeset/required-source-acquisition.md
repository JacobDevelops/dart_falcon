---
default: patch
---

`falcon check` now fails the run (exits nonzero) on unreadable, invalid-UTF-8, or vanished Dart project sources instead of skipping them, and the editor suppresses incomplete cross-file results until sources recover.
