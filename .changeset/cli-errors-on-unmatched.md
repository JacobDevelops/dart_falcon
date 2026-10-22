---
default: minor
---

Report missing input paths and runs that process no files as error diagnostics with a nonzero exit status; use `--no-errors-on-unmatched` to allow these inputs in automation. Package manifests count as processed inputs, and explicit paths excluded by file filters are explained instead of reported as processing no files.
