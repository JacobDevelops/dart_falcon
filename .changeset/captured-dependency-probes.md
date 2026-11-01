---
default: patch
---

Capture unused-dependency resource and build-tool inputs once per analysis environment generation. Malformed or inaccessible inputs warn and conservatively skip only their dependent probe; missing dependency manifests retain their existing behavior. LSP metadata and executable-content changes refresh enabled probes without rebuilding semantic indexes, and hosted dependencies acquire no additional watchers.
