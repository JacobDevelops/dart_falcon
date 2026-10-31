---
default: patch
---

`falcon check` now fails on unreadable required directories, input paths and Dart symlink targets it previously skipped, naming the path and I/O reason. Missing paths and dangling symlink targets remain absent. Unreadable directories outside all positive `files.includes` patterns warn and skip; with default empty includes every directory is potentially required. Exclude unwanted directories explicitly, for example `"files": {"includes": ["**", "!docker/pgdata"]}`. Editor analysis suppresses cross-file results until required discovery recovers, without repeatedly walking or rebuilding the same incomplete project.
