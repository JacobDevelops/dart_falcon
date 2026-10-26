---
default: patch
---

Keep editor analysis responsive with cached workspace parses, reuse project indexes for declaration-preserving body edits, discard superseded diagnostics, analyze initialized workspace folders, and publish merged diagnostics once with UTF-16 positions. Refresh disk sources and discovery through watched-file events rather than scanning file metadata after every edit.

Resolve each editor workspace folder's rules and file includes independently, including configuration reloads and code actions. Report invalid member configurations without discarding valid workspace and sibling configuration.

Cancel queued editor requests before they run and stop ongoing fix-all iterations when the client cancels, without returning stale results.
