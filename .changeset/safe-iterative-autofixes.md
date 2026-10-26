---
default: minor
---

Add iterative safe autofixes with `falcon check --write`, explicit unsafe fixing, and dry-run diffs. Detect conflicting fix cycles and nonconverging growth without limiting large converging files, and reuse external semantic indexes between edits. Configure per-rule fix classifications including path overrides and offer editor quick fixes plus safe fix-all on save, with capability-compatible workspace edits and lazy fix-all resolution. Skip files with syntax errors, symlinks, or concurrent disk changes while still atomically writing converged files. Dry-run reports the original diagnostics and exits nonzero when fixes are pending. Supply safe fixes for unnecessary `new` keywords and convertible quote styles.
