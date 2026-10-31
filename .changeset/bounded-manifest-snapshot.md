---
default: patch
---

Capture bounded package metadata for secure-pubspec-urls and refresh its diagnostics after saved manifest changes. Malformed, unreadable and oversized manifests now produce shared discovery warnings whichever rules are enabled, including manifest-only projects. Only the secure-URL check skips the affected package; other rules still analyze it. Manifests over 4 GiB previously rejected project analysis; oversized manifests now warn and skip optional metadata instead.
