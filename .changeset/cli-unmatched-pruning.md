---
default: minor
---

Make `falcon check` lint the current directory when no paths are supplied. Report missing input paths and runs that process no files as error diagnostics with a nonzero exit status; use `--no-errors-on-unmatched` to allow these inputs in automation. Keep all diagnostics when determining the exit status, so `--max-errors` limits printed output only. Discover Dart sources and package manifests in one directory walk that prunes excluded directories.

Count package manifests as processed inputs, and explain when explicit paths are excluded by file filters instead of reporting that no files were processed.
