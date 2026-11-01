---
default: patch
---

Capture localization metadata only for enabled unused-l10n analysis in shared bounded snapshots. Skip checks with one warning per malformed or unreadable input episode, and refresh declared ARB diagnostics without rebuilding semantic analysis. Support BOM-prefixed ARB templates (previously skipped), preserving original byte spans and report fingerprints while excluding the BOM from UTF-16 editor ranges. Normalize template paths without resolving symlinks, and clear diagnostics when a parent directory is deleted.
