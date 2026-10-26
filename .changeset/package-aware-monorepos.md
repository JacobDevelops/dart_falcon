---
default: minor
---
Detect pub and Melos workspaces, apply member-specific falcon.json configurations in the CLI and language server, and include package identities in workspace diagnostics. Text and SARIF reports include per-package summaries; JSON retains its top-level diagnostic-array contract, with a package name on workspace findings or null for findings outside workspace packages.

Cross-file checks now resolve imports and exports across workspace packages: unused-files and unused-code cover each member's libraries, and no-import-cycles detects cycles spanning sibling packages.
