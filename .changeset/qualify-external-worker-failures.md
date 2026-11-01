---
default: patch
---
Keep unavailable external library parts conservatively unknown, fail CLI analysis after parser or rule worker failures, and qualify LSP publications even for semantic-only analysis while suppressing cross-file results until successful reanalysis. Recover using current sources even when unrelated documents retain earlier failure publications, and clear external worker failures when their import or part directives are removed. Required source failures now report `failed to analyze file` with the path and underlying cause, covering read, invalid-UTF-8 and parser-worker failures without mislabeling worker failures as reads.
