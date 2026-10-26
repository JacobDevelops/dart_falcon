---
default: minor
---

Warnings no longer fail `falcon check` by default. Use `--error-on-warnings` or set `"error-on-warnings": true` in `falcon.json` to restore warning-sensitive CI exits. Errors still fail and info diagnostics never fail.

Add counted, content-based existing-debt baselines with `--write-baseline [PATH]`, `--baseline PATH`, and the `baseline` configuration key. Add SARIF 2.1.0, GitLab Code Quality, Checkstyle XML, Sonar generic issue JSON, and self-contained HTML reports.

Report missing input paths without panicking when a path component is a regular file or canonicalization is otherwise unavailable.

Anchor baseline fingerprints to the workspace, configuration, or Git root so checks from subdirectories match baselines recorded at the root.
