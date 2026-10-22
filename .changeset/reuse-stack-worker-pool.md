---
default: patch
---

Reuse stack-sized workers for parsing and lint rules instead of starting a thread for each pass. Worker panics now produce an analysis diagnostic rather than escaping the worker boundary, while deeply nested Dart code keeps the same stack space.
