---
default: patch
---

Keep editor analysis responsive with cached workspace parses, reuse project indexes for declaration-preserving body edits, discard superseded diagnostics, analyze initialized workspace folders, and publish merged diagnostics once with UTF-16 positions. Refresh disk sources and discovery through watched-file events, with metadata checks of cached parses when rebuilding a snapshot to recover from missed events. Clients without dynamic file watching revalidate closed files on open, save, and quiet analysis boundaries, exponentially backing off unchanged idle scans instead of polling every second. Check configuration metadata before reloading contents.

Resolve each editor workspace folder's rules and file includes independently, including configuration reloads and code actions. Report invalid member configurations without discarding valid workspace and sibling configuration.

Cancel queued editor requests before they run and stop ongoing fix-all iterations when the client cancels, without returning stale results.
