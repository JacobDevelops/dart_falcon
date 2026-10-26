---
default: minor
---
Detect pub and Melos workspaces, apply member-specific falcon.json configurations in the CLI and language server, and include package identities and per-package summaries in workspace reports.

Cross-file checks now resolve imports and exports across workspace packages: unused-files and unused-code cover each member's libraries, and no-import-cycles detects cycles spanning sibling packages.
