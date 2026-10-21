---
default: patch
---

Resolve file filters, exclusions and overrides relative to the configuration file in both the CLI and editor. Single-star globs now stop at directory separators, while double-star globs match nested paths, so `!build/**` excludes build output when checking the current directory.
