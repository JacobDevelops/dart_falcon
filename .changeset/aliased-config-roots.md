---
default: patch
---

Match globs when the configuration root is reached through a directory alias, such as macOS `/var` resolving to `/private/var`, and treat a `~/.falcon.json` reached through a symlinked home directory as user-level settings.
