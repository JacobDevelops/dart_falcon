---
default: patch
---

Reject configuration and package/workspace metadata larger than 16 MiB before parsing or retaining analysis and editor configuration fingerprints, preventing oversized manifests from exhausting memory during CLI and editor analysis.
