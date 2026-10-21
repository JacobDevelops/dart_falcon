---
default: minor
---

Resolve file filters, exclusions and overrides relative to the configuration file in both the CLI and editor. User-level `~/.falcon.json` settings instead resolve globs from the CLI current directory or the editor workspace root. Single-star globs now stop at directory separators, while double-star globs match nested paths, so `!build/**` excludes build output when checking the current directory.
