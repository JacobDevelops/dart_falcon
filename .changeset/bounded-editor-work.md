---
default: patch
---

Coalesce queued document edits, saves, lifecycle transitions and watcher batches without changing the latest diagnostics. Supersede outdated read-only requests while preserving user-action snapshots, keep cancellation and shutdown responsive during bursts, and announce incomplete-analysis warnings only for the current state without losing configuration errors.
