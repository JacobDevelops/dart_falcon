---
default: patch
---

Preserve GestureDetectors with explicit hit-test behavior, and detect tap-only wrappers around resolved Flutter TextButton, ElevatedButton, OutlinedButton and IconButton constructors with inline onPressed callbacks. Keep distinct gestures and nullable child handlers unflagged, and direct tap handling to the button callback.
