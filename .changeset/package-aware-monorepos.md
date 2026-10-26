---
default: minor
---
Detect pub and Melos workspaces, apply member-specific falcon.json configurations in the CLI and language server, and include package identities in workspace diagnostics. Text and SARIF reports include per-package summaries; JSON retains its top-level diagnostic-array contract, with a package name on workspace findings or null for findings outside workspace packages.

Cross-file checks now resolve imports and exports across workspace packages: unused-files and unused-code cover each member's libraries, and no-import-cycles detects cycles spanning sibling packages.

Keep current-directory configuration discovery consistent for default and explicit input paths. Load only selected workspace members' configurations in the CLI; the language server reports malformed member configurations while retaining the base and other valid member configurations.

Warn once per member configuration and explicitly set run-wide option when `max-errors` (or `max_errors`), `error-on-warnings`, or `baseline` is ignored, including explicit default values. Omitted options do not warn.
