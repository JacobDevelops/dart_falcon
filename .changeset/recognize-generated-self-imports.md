---
default: patch
---

Recognize empty import paths as imports of the current library, as used by generated FFI bindings. These imports no longer produce a misleading configuration warning in the command line or editor, and paths that point to directories are treated as absent optional sources.
