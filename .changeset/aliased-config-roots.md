---
default: patch
---

Match globs when the configuration root or one of its subdirectories is reached through a directory alias, such as macOS `/var` resolving to `/private/var`, and treat a `~/.falcon.json` reached through a symlinked home directory as user-level settings. Reuse resolved document directories across file filters and overrides without filesystem lookups for paths inside the lexical root.
