---
default: patch
---

Skip explicitly disabled Flutter buttons in `minimum-touch-target` when `onPressed` is literal null and `onLongPress` is absent or literal null. Dynamic callbacks and active long presses still need a touch target.
