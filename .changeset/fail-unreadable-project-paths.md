---
default: patch
---

`falcon check` now fails on unreadable directories, input paths and Dart symlink targets instead of skipping them, naming the path and I/O error.
